#[cfg(feature = "gbm-probe")]
use super::RealAtomicScanoutRenderDeviceDiscovery;
use super::{
    RealAtomicScanoutCard, RealAtomicScanoutCardSelection, RealAtomicScanoutCardSelectionStatus,
};
use crate::prelude::*;

#[derive(Debug)]
pub struct RealAtomicScanoutPageFlipSession {
    pub(super) card: RealAtomicScanoutCard,
    selections: Vec<LibdrmNativePrimaryPlaneSelection>,
    outputs: Vec<OutputId>,
    heads: Vec<sophia_engine::RenderHeadId>,
    pub(super) reader: NativeLibdrmPageFlipEventReader<RealAtomicScanoutCard>,
    pub(super) poller: NativeLibdrmPageFlipEventPoller,
    #[cfg(feature = "gbm-probe")]
    cursor_buffer: Option<drm::control::dumbbuffer::DumbBuffer>,
    #[cfg(feature = "gbm-probe")]
    cursor_dimensions: Option<LegacyHardwareCursorDimensions>,
    #[cfg(feature = "gbm-probe")]
    cursor_planes: Option<Vec<RealAtomicCursorPlane>>,
    #[cfg(feature = "gbm-probe")]
    cursor_controller: LegacyHardwareCursorController<drm::control::crtc::Handle>,
    #[cfg(feature = "gbm-probe")]
    cursor_crtcs_sanitized: bool,
}

#[cfg(feature = "gbm-probe")]
#[derive(Clone, Debug)]
struct RealAtomicCursorPlane {
    plane: drm::control::plane::Handle,
    fb_id: drm::control::property::Handle,
    crtc_id: drm::control::property::Handle,
}

#[cfg(feature = "gbm-probe")]
struct RealLegacyHardwareCursorDevice<'a> {
    card: &'a RealAtomicScanoutCard,
    buffer: &'a drm::control::dumbbuffer::DumbBuffer,
}

#[cfg(feature = "gbm-probe")]
impl LegacyHardwareCursorDevice for RealLegacyHardwareCursorDevice<'_> {
    type Crtc = drm::control::crtc::Handle;

    #[allow(deprecated)]
    fn hide_cursor(&mut self, crtc: Self::Crtc) -> io::Result<()> {
        use drm::control::Device as _;

        self.card
            .set_cursor::<drm::control::dumbbuffer::DumbBuffer>(crtc, None)
    }

    #[allow(deprecated)]
    fn install_cursor(&mut self, crtc: Self::Crtc) -> io::Result<()> {
        use drm::control::Device as _;

        self.card.set_cursor2(crtc, Some(self.buffer), (0, 0))
    }

    #[allow(deprecated)]
    fn move_cursor(&mut self, crtc: Self::Crtc, x: i32, y: i32) -> io::Result<()> {
        use drm::control::Device as _;

        self.card.move_cursor(crtc, (x, y))
    }
}

impl RealAtomicScanoutPageFlipSession {
    #[cfg(feature = "gbm-probe")]
    fn discover_atomic_cursor_planes(&self) -> io::Result<Vec<RealAtomicCursorPlane>> {
        let mut cursor_planes = Vec::new();
        for plane in LibdrmNativeKmsSelectionDevice::plane_handles(&self.card)? {
            if LibdrmNativeKmsSelectionDevice::plane_type(&self.card, plane)?
                != Some(drm::control::PlaneType::Cursor)
            {
                continue;
            }
            let snapshot = LibdrmNativeKmsSelectionDevice::plane_snapshot(&self.card, plane)?;
            let crtcs = self
                .selections
                .iter()
                .filter_map(|selection| {
                    snapshot
                        .supports_crtc(selection.crtc)
                        .then_some(selection.crtc)
                })
                .collect::<Vec<_>>();
            if crtcs.is_empty() {
                continue;
            }
            let properties =
                LibdrmNativePropertyLookupDevice::plane_property_handles(&self.card, plane)?;
            let required = |name| {
                properties.get(name).ok_or_else(|| {
                    io::Error::other(format!("atomic cursor plane is missing {name}"))
                })
            };
            cursor_planes.push(RealAtomicCursorPlane {
                plane,
                fb_id: required("FB_ID")?,
                crtc_id: required("CRTC_ID")?,
            });
        }
        Ok(cursor_planes)
    }

    #[cfg(feature = "gbm-probe")]
    fn detach_atomic_cursor_planes(&self, planes: &[RealAtomicCursorPlane]) -> io::Result<()> {
        use drm::control::Device as _;

        if planes.is_empty() {
            return Ok(());
        }
        let mut request = drm::control::atomic::AtomicModeReq::new();
        for cursor in planes {
            request.add_property(
                cursor.plane,
                cursor.fb_id,
                drm::control::property::Value::Framebuffer(None),
            );
            request.add_property(
                cursor.plane,
                cursor.crtc_id,
                drm::control::property::Value::CRTC(None),
            );
        }
        self.card
            .atomic_commit(drm::control::AtomicCommitFlags::empty(), request)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn classic_hardware_cursor_initialized(&self) -> bool {
        self.cursor_controller.is_initialized()
    }

    #[cfg(feature = "gbm-probe")]
    pub fn initialize_classic_hardware_cursor(&mut self) -> io::Result<()> {
        use drm::{Device as _, control::Device as _};

        if self.cursor_controller.is_initialized() {
            return Ok(());
        }
        if self.cursor_planes.is_none() {
            self.cursor_planes = Some(self.discover_atomic_cursor_planes()?);
        }
        if !self.cursor_crtcs_sanitized {
            let planes = self
                .cursor_planes
                .as_deref()
                .ok_or_else(|| io::Error::other("atomic cursor planes disappeared"))?;
            self.detach_atomic_cursor_planes(planes)?;
            self.cursor_crtcs_sanitized = true;
        }
        if self.cursor_dimensions.is_none() {
            let width = self
                .card
                .get_driver_capability(drm::DriverCapability::CursorWidth)
                .ok();
            let height = self
                .card
                .get_driver_capability(drm::DriverCapability::CursorHeight)
                .ok();
            self.cursor_dimensions = Some(resolve_legacy_hardware_cursor_dimensions(width, height));
        }
        if self.cursor_buffer.is_none() {
            let dimensions = self
                .cursor_dimensions
                .ok_or_else(|| io::Error::other("hardware cursor dimensions are unavailable"))?;
            let raster_edge = u32::try_from(sophia_renderer_live::DEFAULT_CURSOR_EDGE)
                .map_err(|_| io::Error::other("cursor raster edge exceeds u32"))?;
            if dimensions.width < raster_edge || dimensions.height < raster_edge {
                return Err(io::Error::other(format!(
                    "driver cursor dimensions {}x{} are smaller than the {}x{} cursor raster",
                    dimensions.width, dimensions.height, raster_edge, raster_edge,
                )));
            }
            let mut buffer = self.card.create_dumb_buffer(
                (dimensions.width, dimensions.height),
                drm::buffer::DrmFourcc::Argb8888,
                32,
            )?;
            let pitch = usize::try_from(drm::buffer::Buffer::pitch(&buffer))
                .map_err(|_| io::Error::other("cursor pitch exceeds address space"))?;
            {
                let mut mapping = self.card.map_dumb_buffer(&mut buffer)?;
                mapping.fill(0);
                for (y, row) in sophia_renderer_live::DEFAULT_CURSOR_SHAPE
                    .iter()
                    .enumerate()
                {
                    for (x, pixel) in row.iter().copied().enumerate() {
                        let color = match pixel {
                            b'W' => [0xff, 0xff, 0xff, 0xff],
                            b'#' => [0x00, 0x00, 0x00, 0xff],
                            _ => continue,
                        };
                        let offset = y * pitch + x * 4;
                        mapping[offset..offset + 4].copy_from_slice(&color);
                    }
                }
            }
            self.cursor_buffer = Some(buffer);
        }

        let crtcs = self
            .selections
            .iter()
            .map(|selection| selection.crtc)
            .collect::<Vec<_>>();
        let buffer = self
            .cursor_buffer
            .as_ref()
            .ok_or_else(|| io::Error::other("legacy hardware cursor buffer is unavailable"))?;
        let mut device = RealLegacyHardwareCursorDevice {
            card: &self.card,
            buffer,
        };
        self.cursor_controller.initialize(&mut device, &crtcs)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn update_classic_hardware_cursor(
        &mut self,
        target: Option<(LibdrmNativePrimaryPlaneSelection, i32, i32)>,
    ) -> io::Result<ClassicHardwareCursorUpdate> {
        let target = target
            .filter(|(selection, _, _)| {
                self.selections
                    .iter()
                    .any(|candidate| candidate.crtc == selection.crtc)
            })
            .map(|(selection, x, y)| LegacyHardwareCursorTarget {
                crtc: selection.crtc,
                x,
                y,
            });
        let buffer = self
            .cursor_buffer
            .as_ref()
            .ok_or_else(|| io::Error::other("legacy hardware cursor buffer is unavailable"))?;
        let mut device = RealLegacyHardwareCursorDevice {
            card: &self.card,
            buffer,
        };
        self.cursor_controller.update(&mut device, target)
    }

    #[cfg(feature = "gbm-probe")]
    pub fn update_classic_hardware_cursors(
        &mut self,
        targets: &[(LibdrmNativePrimaryPlaneSelection, i32, i32)],
    ) -> io::Result<ClassicHardwareCursorUpdate> {
        let targets = targets
            .iter()
            .filter(|(selection, _, _)| {
                self.selections
                    .iter()
                    .any(|candidate| candidate.crtc == selection.crtc)
            })
            .map(|(selection, x, y)| LegacyHardwareCursorTarget {
                crtc: selection.crtc,
                x: *x,
                y: *y,
            })
            .collect::<Vec<_>>();
        let buffer = self
            .cursor_buffer
            .as_ref()
            .ok_or_else(|| io::Error::other("legacy hardware cursor buffer is unavailable"))?;
        let mut device = RealLegacyHardwareCursorDevice {
            card: &self.card,
            buffer,
        };
        self.cursor_controller.update_many(&mut device, &targets)
    }

    pub fn card(&self) -> &RealAtomicScanoutCard {
        &self.card
    }

    pub fn selection(&self) -> LibdrmNativePrimaryPlaneSelection {
        self.selections[0]
    }

    pub fn selections(&self) -> &[LibdrmNativePrimaryPlaneSelection] {
        &self.selections
    }

    pub fn heads(&self) -> &[sophia_engine::RenderHeadId] {
        &self.heads
    }

    pub fn outputs(&self) -> &[OutputId] {
        &self.outputs
    }

    pub fn output_capabilities(&self) -> io::Result<Vec<LibdrmNativeOutputCapability>> {
        self.selections
            .iter()
            .copied()
            .zip(self.outputs.iter().copied())
            .map(|(selection, output)| read_native_output_capability(&self.card, selection, output))
            .collect()
    }

    pub fn vrr_properties_for_selection(
        &self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> LibdrmNativeVrrPropertyDiscoveryResult {
        discover_native_vrr_properties(&self.card, selection.connector, selection.crtc)
    }

    pub fn property_names_for_selection(
        &self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> io::Result<(Vec<String>, Vec<String>)> {
        let mut connector = self
            .card
            .connector_property_handles(selection.connector)?
            .names()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut crtc = self
            .card
            .crtc_property_handles(selection.crtc)?
            .names()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        connector.sort();
        crtc.sort();
        Ok((connector, crtc))
    }

    #[cfg(feature = "gbm-probe")]
    pub fn render_device_discovery(&self) -> io::Result<RealAtomicScanoutRenderDeviceDiscovery> {
        RealAtomicScanoutRenderDeviceDiscovery::from_card(&self.card)
    }

    #[cfg(all(feature = "gbm-probe", feature = "libdrm-events"))]
    pub fn preferred_xrgb8888_scanout_modifiers(&self) -> Vec<u64> {
        self.preferred_xrgb8888_scanout_modifiers_for_selection(self.selection())
    }

    #[cfg(all(feature = "gbm-probe", feature = "libdrm-events"))]
    pub fn preferred_xrgb8888_scanout_modifiers_for_selection(
        &self,
        selection: LibdrmNativePrimaryPlaneSelection,
    ) -> Vec<u64> {
        let discovery = discover_native_primary_plane_property_handles(
            &self.card,
            selection.connector,
            selection.crtc,
            selection.plane,
        );
        let Some(properties) = discovery.properties else {
            return Vec::new();
        };
        let Some(in_formats) = properties.plane_in_formats() else {
            return Vec::new();
        };

        let Ok(plane_properties) =
            drm::control::Device::get_properties(&self.card, selection.plane)
        else {
            return Vec::new();
        };
        let Some(blob_id) = plane_properties
            .iter()
            .find_map(|(property, value)| (*property == in_formats).then_some(*value))
        else {
            return Vec::new();
        };
        if blob_id == 0 {
            return Vec::new();
        }

        let Ok(blob) = drm::control::Device::get_property_blob(&self.card, blob_id) else {
            return Vec::new();
        };
        let parsed = LibdrmNativePlaneFormatModifierTable::parse_for_format(
            &blob,
            drm::buffer::DrmFourcc::Xrgb8888,
        );
        let Some(table) = parsed.table else {
            return Vec::new();
        };

        table.modifiers().iter().copied().map(u64::from).collect()
    }

    #[cfg(all(feature = "gbm-probe", feature = "libinput-events"))]
    pub fn run_tick_with_native_gbm_rendered_primary_plane_scanout<P, E>(
        &mut self,
        runtime: &mut LiveBackendRuntimeAssembly<LiveInputReadinessGatedPoller<P>>,
        input: CompositorBackendTickInput,
        readiness: LiveBackendSessionLoopReadiness,
        page_flip_budget: LiveBackendSessionLoopPageFlipBudget,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<E>,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
    ) -> Result<LiveBackendSessionLoopTickReport, CompositorBackendAssemblyError>
    where
        P: NonBlockingInputPoller,
        E: RenderDeviceDiscoveryBackend,
    {
        runtime.run_session_loop_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_and_native_page_flip_events_with(
            input,
            readiness,
            page_flip_budget,
            &self.card,
            exporter,
            &mut self.reader,
            &mut self.poller,
            sender,
        )
    }

    #[cfg(all(feature = "gbm-probe", feature = "libdrm-events"))]
    #[allow(clippy::too_many_arguments)]
    pub fn run_native_gbm_runtime_tick<P, E>(
        &mut self,
        runtime: &mut LiveBackendRuntimeAssembly<P>,
        input: CompositorBackendTickInput,
        exporter: &mut NativeGbmRenderedScanoutBufferDiscoveryExporter<E>,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
        max_read: usize,
        max_emit: usize,
    ) -> Result<LiveBackendRuntimeNativePageFlipTickReport, CompositorBackendAssemblyError>
    where
        P: NonBlockingInputPoller,
        E: RenderDeviceDiscoveryBackend,
    {
        runtime
            .run_tick_with_native_gbm_rendered_primary_plane_scanout_exporter_and_native_page_flip_events_with(
                input,
                &self.card,
                exporter,
                &mut self.reader,
                &mut self.poller,
                sender,
                max_read,
                max_emit,
            )
    }

    #[cfg(feature = "libdrm-events")]
    pub fn poll_native_page_flip_events(
        &mut self,
        sender: &std::sync::mpsc::SyncSender<LivePageFlipCallback>,
        max_read: usize,
        max_emit: usize,
    ) -> LibdrmNativeReadAndPollReport {
        self.poller
            .read_and_poll_page_flip_events(&mut self.reader, sender, max_read, max_emit)
    }

    #[cfg(feature = "libdrm-events")]
    pub fn drain_emitted_kernel_page_flip_timestamps(
        &mut self,
    ) -> Vec<LibdrmKernelPageFlipTimestamp> {
        self.poller.drain_emitted_kernel_timestamps()
    }

    /// What the event poller holds and last saw, for stall attribution.
    ///
    /// A hard stall has two possible authors: an event the kernel never
    /// delivered, and an event delivered but stuck or dropped on the way
    /// through. The poller's pending depth, route count, and last read status
    /// are what separate them at the moment the stall is declared.
    #[cfg(feature = "libdrm-events")]
    pub fn page_flip_poller_diagnostics(&self) -> LibdrmNativePollerDiagnostics {
        self.poller.diagnostics()
    }
}

impl Drop for RealAtomicScanoutPageFlipSession {
    fn drop(&mut self) {
        #[cfg(feature = "gbm-probe")]
        {
            use drm::control::Device as _;
            if let Some(buffer) = self.cursor_buffer.as_ref() {
                let mut device = RealLegacyHardwareCursorDevice {
                    card: &self.card,
                    buffer,
                };
                let _ = self.cursor_controller.hide_for_teardown(&mut device);
            }
            if let Some(buffer) = self.cursor_buffer.take() {
                let _ = self.card.destroy_dumb_buffer(buffer);
            }
        }
    }
}

#[derive(Debug)]
pub struct RealAtomicScanoutPageFlipSessionResult {
    pub status: RealAtomicScanoutPageFlipSessionStatus,
    pub card_selection_status: RealAtomicScanoutCardSelectionStatus,
    pub session: Option<RealAtomicScanoutPageFlipSession>,
}

impl RealAtomicScanoutPageFlipSessionResult {
    pub fn failure_evidence(&self) -> Option<LibdrmNativeAtomicScanoutSmokeEvidence> {
        match self.status {
            RealAtomicScanoutPageFlipSessionStatus::Ready => None,
            RealAtomicScanoutPageFlipSessionStatus::CardSelectionFailed => {
                Some(self.card_selection_status.failure_evidence())
            }
            RealAtomicScanoutPageFlipSessionStatus::CardCloneFailed => {
                let mut evidence = LibdrmNativeAtomicScanoutSmokeEvidence::kms_selection_failed();
                evidence.status = LibdrmNativeAtomicScanoutSmokeStatus::PageFlipReaderUnavailable;
                Some(evidence)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealAtomicScanoutPageFlipSessionStatus {
    Ready,
    CardSelectionFailed,
    CardCloneFailed,
}

impl RealAtomicScanoutCardSelection {
    pub fn into_page_flip_session(
        mut self,
        slot: LibdrmNativeOutputSlot,
        output: OutputId,
        head: sophia_engine::RenderHeadId,
        authority: LibdrmBackendFdAuthority,
    ) -> RealAtomicScanoutPageFlipSessionResult {
        let Some(card) = self.card.take() else {
            return RealAtomicScanoutPageFlipSessionResult {
                status: RealAtomicScanoutPageFlipSessionStatus::CardSelectionFailed,
                card_selection_status: self.status,
                session: None,
            };
        };
        let Some(selection) = self.selection else {
            return RealAtomicScanoutPageFlipSessionResult {
                status: RealAtomicScanoutPageFlipSessionStatus::CardSelectionFailed,
                card_selection_status: self.status,
                session: None,
            };
        };
        if self.status != RealAtomicScanoutCardSelectionStatus::Selected {
            return RealAtomicScanoutPageFlipSessionResult {
                status: RealAtomicScanoutPageFlipSessionStatus::CardSelectionFailed,
                card_selection_status: self.status,
                session: None,
            };
        };

        let Ok(reader_card) = card.try_clone() else {
            return RealAtomicScanoutPageFlipSessionResult {
                status: RealAtomicScanoutPageFlipSessionStatus::CardCloneFailed,
                card_selection_status: self.status,
                session: None,
            };
        };
        let reader = NativeLibdrmPageFlipEventReader::new(reader_card)
            .with_crtc_routes([selection.crtc_route(slot)]);
        let poller = NativeLibdrmPageFlipEventPoller::new(
            LibdrmNativePageFlipSource::from_authority(authority),
        )
        .with_routes([LibdrmNativeOutputRoute { slot, output, head }]);

        RealAtomicScanoutPageFlipSessionResult {
            status: RealAtomicScanoutPageFlipSessionStatus::Ready,
            card_selection_status: self.status,
            session: Some(RealAtomicScanoutPageFlipSession {
                card,
                selections: vec![selection],
                outputs: vec![output],
                heads: vec![head],
                reader,
                poller,
                #[cfg(feature = "gbm-probe")]
                cursor_buffer: None,
                #[cfg(feature = "gbm-probe")]
                cursor_dimensions: None,
                #[cfg(feature = "gbm-probe")]
                cursor_planes: None,
                #[cfg(feature = "gbm-probe")]
                cursor_controller: LegacyHardwareCursorController::default(),
                #[cfg(feature = "gbm-probe")]
                cursor_crtcs_sanitized: false,
            }),
        }
    }
}

#[derive(Debug)]
pub struct RealAtomicScanoutPageFlipSessionSetResult {
    pub status: RealAtomicScanoutPageFlipSessionSetStatus,
    pub sessions: Vec<RealAtomicScanoutPageFlipSession>,
    pub output_count: usize,
    /// The backend head table for the built sessions: which card, connector,
    /// and CRTC each minted head names. Physical identity stays here; the
    /// sessions and routes above carry only the opaque head.
    pub head_records: Vec<crate::LiveNativeHeadRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealAtomicScanoutPageFlipSessionSetStatus {
    Ready,
    SelectionFailed,
    CardCloneFailed,
    CapacityExceeded,
}

impl RealAtomicScanoutSelectionSet {
    pub fn into_page_flip_sessions(
        self,
        authority: LibdrmBackendFdAuthority,
    ) -> RealAtomicScanoutPageFlipSessionSetResult {
        self.into_page_flip_sessions_with_mirroring(authority, &NativeMirrorGrouping::none())
    }

    /// Builds sessions, giving every connector in a mirror group one logical output.
    ///
    /// Slots stay per connector because each head has its own page flip to intake;
    /// only the output identity is shared. That is what makes a mirror group one
    /// `SnapshotOutput` to policy while remaining N heads to the kernel.
    pub fn into_page_flip_sessions_with_mirroring(
        self,
        authority: LibdrmBackendFdAuthority,
        grouping: &NativeMirrorGrouping,
    ) -> RealAtomicScanoutPageFlipSessionSetResult {
        if self.status != RealAtomicScanoutSelectionSetStatus::SelectedAll {
            return RealAtomicScanoutPageFlipSessionSetResult {
                status: RealAtomicScanoutPageFlipSessionSetStatus::SelectionFailed,
                sessions: Vec::new(),
                output_count: 0,
                head_records: Vec::new(),
            };
        }
        let mut sessions = Vec::new();
        let mut head_records = Vec::new();
        let mut allocator = sophia_engine::RenderHeadAllocator::new();
        let mut next_output = 1u64;
        let mut next_slot = 1u16;
        // Logical outputs already handed to a mirror group, so its later members
        // join it instead of taking a fresh identity.
        let mut group_outputs: BTreeMap<usize, OutputId> = BTreeMap::new();
        for (card_index, target_set) in self.cards.into_iter().enumerate() {
            let Ok(reader_card) = target_set.card.try_clone() else {
                return RealAtomicScanoutPageFlipSessionSetResult {
                    status: RealAtomicScanoutPageFlipSessionSetStatus::CardCloneFailed,
                    sessions: Vec::new(),
                    output_count: 0,
                    head_records: Vec::new(),
                };
            };
            let mut crtc_routes = Vec::new();
            let mut output_routes = Vec::new();
            let mut outputs = Vec::new();
            let mut heads = Vec::new();
            for selection in target_set.selections.iter().copied() {
                let Some(slot) = LibdrmNativeOutputSlot::new(next_slot) else {
                    return RealAtomicScanoutPageFlipSessionSetResult {
                        status: RealAtomicScanoutPageFlipSessionSetStatus::CapacityExceeded,
                        sessions: Vec::new(),
                        output_count: 0,
                        head_records: Vec::new(),
                    };
                };
                // The card answers the connector's name directly, which is what
                // configuration named it. Resolving here rather than upstream is
                // what keeps the grouping free of the id-to-name circularity.
                let connector_name = drm::control::Device::get_connector(
                    &target_set.card,
                    selection.connector_handle(),
                    false,
                )
                .map(|connector| connector.to_string())
                .unwrap_or_default();
                let group = grouping.group_of(&connector_name);
                let output = match group.and_then(|group| group_outputs.get(&group).copied()) {
                    Some(output) => output,
                    None => {
                        let output = OutputId::from_raw(next_output);
                        next_output = next_output.saturating_add(1);
                        if let Some(group) = group {
                            group_outputs.insert(group, output);
                        }
                        output
                    }
                };
                let head = allocator.mint();
                crtc_routes.push(selection.crtc_route(slot));
                output_routes.push(LibdrmNativeOutputRoute { slot, output, head });
                outputs.push(output);
                heads.push(head);
                head_records.push(crate::LiveNativeHeadRecord {
                    head,
                    output,
                    card_index,
                    connector_id: selection.connector_id(),
                    crtc_id: selection.crtc_id(),
                    connector_name,
                });
                next_slot = next_slot.saturating_add(1);
            }
            let reader =
                NativeLibdrmPageFlipEventReader::new(reader_card).with_crtc_routes(crtc_routes);
            let poller = NativeLibdrmPageFlipEventPoller::new(
                LibdrmNativePageFlipSource::from_authority(authority),
            )
            .with_routes(output_routes);
            sessions.push(RealAtomicScanoutPageFlipSession {
                card: target_set.card,
                selections: target_set.selections,
                outputs,
                heads,
                reader,
                poller,
                #[cfg(feature = "gbm-probe")]
                cursor_buffer: None,
                #[cfg(feature = "gbm-probe")]
                cursor_dimensions: None,
                #[cfg(feature = "gbm-probe")]
                cursor_planes: None,
                #[cfg(feature = "gbm-probe")]
                cursor_controller: LegacyHardwareCursorController::default(),
                #[cfg(feature = "gbm-probe")]
                cursor_crtcs_sanitized: false,
            });
        }
        let output_count = usize::try_from(next_output.saturating_sub(1)).unwrap_or(usize::MAX);
        RealAtomicScanoutPageFlipSessionSetResult {
            status: RealAtomicScanoutPageFlipSessionSetStatus::Ready,
            sessions,
            output_count,
            head_records,
        }
    }
}
