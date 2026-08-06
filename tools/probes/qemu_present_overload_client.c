#define _POSIX_C_SOURCE 200809L

#include <errno.h>
#include <gbm.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <unistd.h>
#include <xcb/dri3.h>
#include <xcb/present.h>
#include <xcb/xcb.h>

enum {
    BUFFER_COUNT = 3,
    WINDOW_WIDTH = 636,
    WINDOW_HEIGHT = 796,
    PRESENT_INTERVAL_USEC = 5000,
};

struct present_buffer {
    struct gbm_bo *bo;
    xcb_pixmap_t pixmap;
};

static volatile sig_atomic_t running = 1;

static void stop(int signal_number)
{
    (void)signal_number;
    running = 0;
}

static int fill_buffer(struct gbm_bo *bo, uint32_t color)
{
    uint32_t map_stride;
    void *map_data = NULL;
    uint8_t *bytes;
    uint32_t x;
    uint32_t y;

    bytes = gbm_bo_map(
        bo,
        0,
        0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        GBM_BO_TRANSFER_WRITE,
        &map_stride,
        &map_data);
    if (bytes == NULL)
        return -1;
    for (y = 0; y < WINDOW_HEIGHT; y++) {
        uint32_t *row = (uint32_t *)(bytes + y * map_stride);
        for (x = 0; x < WINDOW_WIDTH; x++)
            row[x] = color ^ ((x / 32 + y / 32) & 1 ? 0x00101010u : 0);
    }
    gbm_bo_unmap(bo, map_data);
    return 0;
}

static int create_present_buffer(
    xcb_connection_t *connection,
    xcb_drawable_t drawable,
    struct gbm_device *device,
    struct present_buffer *buffer,
    uint32_t color)
{
    xcb_generic_error_t *error;
    xcb_void_cookie_t cookie;
    uint32_t stride;
    uint64_t size;
    int dma_buf_fd;

    buffer->bo = gbm_bo_create(
        device,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        GBM_FORMAT_XRGB8888,
        GBM_BO_USE_RENDERING | GBM_BO_USE_LINEAR);
    if (buffer->bo == NULL || fill_buffer(buffer->bo, color) != 0)
        return -1;
    stride = gbm_bo_get_stride(buffer->bo);
    size = (uint64_t)stride * WINDOW_HEIGHT;
    if (stride > UINT16_MAX || size > UINT32_MAX)
        return -1;
    dma_buf_fd = gbm_bo_get_fd(buffer->bo);
    if (dma_buf_fd < 0)
        return -1;

    buffer->pixmap = xcb_generate_id(connection);
    cookie = xcb_dri3_pixmap_from_buffer_checked(
        connection,
        buffer->pixmap,
        drawable,
        (uint32_t)size,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        (uint16_t)stride,
        24,
        32,
        dma_buf_fd);
    error = xcb_request_check(connection, cookie);
    if (error != NULL) {
        fprintf(stderr, "overload client DRI3 pixmap failed: error=%u\n", error->error_code);
        free(error);
        return -1;
    }
    return 0;
}

int main(void)
{
    static const uint32_t colors[BUFFER_COUNT] = {
        0x00cc3030u,
        0x0030cc30u,
        0x003030ccu,
    };
    struct present_buffer buffers[BUFFER_COUNT] = {0};
    struct timespec interval = {
        .tv_sec = 0,
        .tv_nsec = PRESENT_INTERVAL_USEC * 1000,
    };
    xcb_connection_t *connection;
    xcb_screen_iterator_t screens;
    xcb_screen_t *screen;
    xcb_dri3_open_reply_t *open_reply;
    xcb_generic_error_t *error = NULL;
    struct gbm_device *device;
    xcb_present_event_t present_event;
    xcb_window_t window;
    uint32_t window_values[2];
    uint32_t serial = 1;
    int drm_fd;
    int index;

    setvbuf(stdout, NULL, _IOLBF, 0);
    signal(SIGINT, stop);
    signal(SIGTERM, stop);
    signal(SIGHUP, stop);

    connection = xcb_connect(NULL, NULL);
    if (connection == NULL || xcb_connection_has_error(connection) != 0) {
        fputs("overload client could not connect to DISPLAY\n", stderr);
        return 1;
    }
    screens = xcb_setup_roots_iterator(xcb_get_setup(connection));
    screen = screens.data;
    if (screen == NULL) {
        fputs("overload client found no X screen\n", stderr);
        return 1;
    }

    open_reply = xcb_dri3_open_reply(
        connection,
        xcb_dri3_open(connection, screen->root, 0),
        &error);
    if (error != NULL || open_reply == NULL || open_reply->nfd != 1) {
        fputs("overload client could not acquire the DRI3 render node\n", stderr);
        free(error);
        free(open_reply);
        return 1;
    }
    drm_fd = xcb_dri3_open_reply_fds(connection, open_reply)[0];
    free(open_reply);
    device = gbm_create_device(drm_fd);
    if (device == NULL) {
        fputs("overload client could not create a GBM device\n", stderr);
        close(drm_fd);
        return 1;
    }

    window = xcb_generate_id(connection);
    window_values[0] = screen->black_pixel;
    window_values[1] = XCB_EVENT_MASK_STRUCTURE_NOTIFY;
    xcb_create_window(
        connection,
        screen->root_depth,
        window,
        screen->root,
        0,
        0,
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        0,
        XCB_WINDOW_CLASS_INPUT_OUTPUT,
        screen->root_visual,
        XCB_CW_BACK_PIXEL | XCB_CW_EVENT_MASK,
        window_values);
    xcb_map_window(connection, window);
    present_event = xcb_generate_id(connection);
    error = xcb_request_check(
        connection,
        xcb_present_select_input_checked(
            connection,
            present_event,
            window,
            XCB_PRESENT_EVENT_MASK_COMPLETE_NOTIFY |
                XCB_PRESENT_EVENT_MASK_IDLE_NOTIFY));
    if (error != NULL) {
        fprintf(
            stderr,
            "overload client could not select Present feedback: error=%u\n",
            error->error_code);
        free(error);
        return 1;
    }
    xcb_flush(connection);

    for (index = 0; index < BUFFER_COUNT; index++) {
        if (create_present_buffer(
                connection,
                window,
                device,
                &buffers[index],
                colors[index]) != 0) {
            fputs("overload client could not create its bounded buffer pool\n", stderr);
            return 1;
        }
    }
    xcb_flush(connection);
    printf(
        "sophia_qemu_overload_client schema=1 status=running buffers=%d interval_usec=%d feedback=complete-idle\n",
        BUFFER_COUNT,
        PRESENT_INTERVAL_USEC);

    while (running && xcb_connection_has_error(connection) == 0) {
        struct present_buffer *buffer = &buffers[serial % BUFFER_COUNT];
        xcb_generic_event_t *event;

        // Drain feedback before producing more work so the proof exercises the
        // client-visible route without turning its socket into another queue.
        while ((event = xcb_poll_for_event(connection)) != NULL)
            free(event);

        xcb_present_pixmap(
            connection,
            window,
            buffer->pixmap,
            serial,
            XCB_NONE,
            XCB_NONE,
            0,
            0,
            XCB_NONE,
            XCB_NONE,
            XCB_NONE,
            XCB_PRESENT_OPTION_ASYNC,
            0,
            0,
            0,
            0,
            NULL);
        xcb_flush(connection);
        serial++;
        while (nanosleep(&interval, &interval) != 0 && errno == EINTR && running) {}
        interval.tv_sec = 0;
        interval.tv_nsec = PRESENT_INTERVAL_USEC * 1000;
    }

    xcb_disconnect(connection);
    for (index = 0; index < BUFFER_COUNT; index++) {
        if (buffers[index].bo != NULL)
            gbm_bo_destroy(buffers[index].bo);
    }
    gbm_device_destroy(device);
    close(drm_fd);
    return 0;
}
