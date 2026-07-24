#[cfg(unix)]
impl From<crate::XAuthorityTransportError> for X11SetupSocketError {
    fn from(error: crate::XAuthorityTransportError) -> Self {
        Self::new(error.to_string())
    }
}

#[cfg(unix)]
pub fn read_x11_setup_request(
    stream: &mut UnixStream,
) -> Result<XSetupRequest, X11SetupSocketError> {
    let mut bytes = vec![0; X_SETUP_CLIENT_PREFIX_LEN];
    stream.read_exact(&mut bytes).map_err(|error| {
        X11SetupSocketError::new(format!("failed to read X11 setup prefix: {error}"))
    })?;
    let total_len = x11_setup_request_total_len(&bytes)
        .map_err(|error| X11SetupSocketError::new(format!("invalid X11 setup prefix: {error}")))?;
    bytes.resize(total_len, 0);
    stream
        .read_exact(&mut bytes[X_SETUP_CLIENT_PREFIX_LEN..])
        .map_err(|error| {
            X11SetupSocketError::new(format!("failed to read X11 setup auth fields: {error}"))
        })?;
    parse_x11_setup_request(&bytes)
        .map_err(|error| X11SetupSocketError::new(format!("invalid X11 setup request: {error}")))
}

/// Send one X11 output record while attaching its descriptors exactly once.
///
/// `SCM_RIGHTS` accompanies the first successful byte range. If the stream
/// accepts only part of the byte payload, the remainder is written without
/// ancillary data so the receiver cannot observe duplicate descriptors.
#[cfg(unix)]
pub fn write_x11_socket_output_record(
    stream: &mut UnixStream,
    record: X11SocketOutputRecord,
) -> std::io::Result<()> {
    let X11SocketOutputRecord { bytes, fds } = record;
    if fds.is_empty() {
        return stream.write_all(&bytes);
    }

    let borrowed = fds.iter().map(AsFd::as_fd).collect::<Vec<_>>();
    let mut ancillary_space = [MaybeUninit::uninit();
        rustix::cmsg_space!(ScmRights(sophia_protocol::DMA_BUF_MAX_PLANES))];
    let mut ancillary = rustix::net::SendAncillaryBuffer::new(&mut ancillary_space);
    if !ancillary.push(rustix::net::SendAncillaryMessage::ScmRights(&borrowed)) {
        return Err(std::io::Error::other(
            "failed to encode X11 output file descriptors",
        ));
    }

    let sent = loop {
        match rustix::net::sendmsg(
            &*stream,
            &[IoSlice::new(&bytes)],
            &mut ancillary,
            rustix::net::SendFlags::empty(),
        ) {
            Ok(sent) => break sent,
            Err(error) => {
                let error = std::io::Error::from(error);
                if error.kind() == ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
        }
    };
    if sent == 0 {
        return Err(std::io::Error::new(
            ErrorKind::WriteZero,
            "failed to write X11 output record",
        ));
    }
    stream.write_all(&bytes[sent..])
}

#[cfg(unix)]
#[derive(Debug)]
pub struct X11ReceivedCoreRequest {
    pub major_opcode: u8,
    pub bytes: Vec<u8>,
    pub fds: Vec<OwnedFd>,
}

pub fn read_x11_core_request(
    stream: &mut UnixStream,
    byte_order: crate::XByteOrder,
) -> Result<Option<X11ReceivedCoreRequest>, X11SetupSocketError> {
    let mut header = [0; 4];
    let mut ancillary_space = [MaybeUninit::uninit();
        rustix::cmsg_space!(ScmRights(sophia_protocol::DMA_BUF_MAX_PLANES))];
    let mut ancillary = rustix::net::RecvAncillaryBuffer::new(&mut ancillary_space);
    let mut iov = [IoSliceMut::new(&mut header)];
    let received = match rustix::net::recvmsg(
        &*stream,
        &mut iov,
        &mut ancillary,
        rustix::net::RecvFlags::CMSG_CLOEXEC,
    ) {
        Ok(received) => received,
        Err(error) => {
            let error = std::io::Error::from(error);
            if matches!(
                error.kind(),
                ErrorKind::UnexpectedEof
                    | ErrorKind::ConnectionReset
                    | ErrorKind::TimedOut
                    | ErrorKind::WouldBlock
            ) {
                return Ok(None);
            }
            return Err(X11SetupSocketError::new(format!(
                "failed to read X11 request header: {error}"
            )));
        }
    };
    if received.bytes == 0 {
        return Ok(None);
    }
    if received.flags.contains(rustix::net::ReturnFlags::CTRUNC) {
        return Err(X11SetupSocketError::new(
            "X11 request carried too many ancillary file descriptors",
        ));
    }
    let mut fds = Vec::new();
    for message in ancillary.drain() {
        if let rustix::net::RecvAncillaryMessage::ScmRights(rights) = message {
            fds.extend(rights);
        }
    }
    if fds.len() > sophia_protocol::DMA_BUF_MAX_PLANES {
        return Err(X11SetupSocketError::new(
            "X11 request carried too many file descriptors",
        ));
    }
    if received.bytes < header.len() {
        match stream.read_exact(&mut header[received.bytes..]) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::UnexpectedEof
                        | ErrorKind::ConnectionReset
                        | ErrorKind::TimedOut
                        | ErrorKind::WouldBlock
                ) =>
            {
                return Ok(None);
            }
            Err(error) => {
                return Err(X11SetupSocketError::new(format!(
                    "failed to read X11 request header: {error}"
                )));
            }
        }
    }

    let length = usize::from(byte_order.u16(&header[2..4])) * 4;
    if length < 4 {
        return Ok(Some(X11ReceivedCoreRequest {
            major_opcode: header[0],
            bytes: header.to_vec(),
            fds,
        }));
    }
    // The setup reply advertises the full core u16 request-length range. Keep
    // the socket reader consistent with that wire contract: Firefox emits
    // large, but still ordinary, requests just below the 65,535-unit limit.
    // BIG-REQUESTS extended (zero u16 plus u32 length) frames remain outside
    // this bounded reader until a captured client requires them.
    let max_len = usize::from(crate::X_SETUP_DEFAULT_MAX_REQUEST_UNITS) * 4;
    if length > max_len {
        return Err(X11SetupSocketError::new(format!(
            "X11 request payload too large: {length}"
        )));
    }

    let mut request = Vec::with_capacity(length);
    request.extend_from_slice(&header);
    request.resize(length, 0);
    stream.read_exact(&mut request[4..]).map_err(|error| {
        X11SetupSocketError::new(format!("failed to read X11 request payload: {error}"))
    })?;

    Ok(Some(X11ReceivedCoreRequest {
        major_opcode: header[0],
        bytes: request,
        fds,
    }))
}
