#include <errno.h>
#include <inttypes.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
#include <xcb/present.h>
#include <xcb/randr.h>
#include <xcb/xcb.h>

enum {
    MAX_TARGET_WINDOWS = 16,
    CHILD_POLL_MSEC = 20,
    DRAIN_MSEC = 250,
};

struct target_window {
    xcb_window_t window;
    xcb_present_event_t event;
};

struct output_geometry {
    uint16_t width;
    uint16_t height;
    const char *source;
};

static uint64_t monotonic_msec(void)
{
    struct timespec now;

    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0)
        return 0;
    return (uint64_t)now.tv_sec * 1000 + (uint64_t)now.tv_nsec / 1000000;
}

static const char *present_mode_name(uint8_t mode)
{
    switch (mode) {
    case XCB_PRESENT_COMPLETE_MODE_COPY:
        return "Copy";
    case XCB_PRESENT_COMPLETE_MODE_FLIP:
        return "Flip";
    case XCB_PRESENT_COMPLETE_MODE_SKIP:
        return "Skip";
    case XCB_PRESENT_COMPLETE_MODE_SUBOPTIMAL_COPY:
        return "SuboptimalCopy";
    default:
        return "Unknown";
    }
}

static int target_index(
    const struct target_window *targets,
    size_t target_count,
    xcb_window_t window)
{
    size_t index;

    for (index = 0; index < target_count; index++) {
        if (targets[index].window == window)
            return (int)index;
    }
    return -1;
}

static int attach_target(
    xcb_connection_t *connection,
    struct target_window *targets,
    size_t *target_count,
    xcb_window_t window)
{
    xcb_generic_error_t *error;
    xcb_void_cookie_t cookie;
    xcb_present_event_t event;

    if (target_index(targets, *target_count, window) >= 0)
        return 0;
    if (*target_count >= MAX_TARGET_WINDOWS) {
        fprintf(stderr, "xserver Present probe exceeded target-window bound\n");
        return -1;
    }

    event = xcb_generate_id(connection);
    cookie = xcb_present_select_input_checked(
        connection,
        event,
        window,
        XCB_PRESENT_EVENT_MASK_COMPLETE_NOTIFY);
    error = xcb_request_check(connection, cookie);
    if (error != NULL) {
        fprintf(
            stderr,
            "xserver Present probe could not observe a candidate window: error=%u\n",
            error->error_code);
        free(error);
        return -1;
    }

    targets[*target_count].window = window;
    targets[*target_count].event = event;
    (*target_count)++;
    printf(
        "xserver_present_target schema=1 status=attached width_match=true "
        "target_count=%zu\n",
        *target_count);
    return 0;
}

static int64_t overlap_area(
    int32_t left_a,
    int32_t top_a,
    int32_t right_a,
    int32_t bottom_a,
    int32_t left_b,
    int32_t top_b,
    int32_t right_b,
    int32_t bottom_b)
{
    int32_t width = (right_a < right_b ? right_a : right_b) -
                    (left_a > left_b ? left_a : left_b);
    int32_t height = (bottom_a < bottom_b ? bottom_a : bottom_b) -
                     (top_a > top_b ? top_a : top_b);

    if (width <= 0 || height <= 0)
        return 0;
    return (int64_t)width * height;
}

static struct output_geometry output_for_window(
    xcb_connection_t *connection,
    xcb_window_t root,
    xcb_window_t window,
    const xcb_screen_t *screen)
{
    struct output_geometry output = {
        .width = screen->width_in_pixels,
        .height = screen->height_in_pixels,
        .source = "root",
    };
    xcb_get_geometry_reply_t *geometry;
    xcb_translate_coordinates_reply_t *translated;
    xcb_randr_get_monitors_reply_t *monitors;
    xcb_randr_monitor_info_iterator_t iterator;
    int64_t best_area = 0;

    geometry = xcb_get_geometry_reply(
        connection, xcb_get_geometry(connection, window), NULL);
    translated = xcb_translate_coordinates_reply(
        connection,
        xcb_translate_coordinates(connection, window, root, 0, 0),
        NULL);
    monitors = xcb_randr_get_monitors_reply(
        connection, xcb_randr_get_monitors(connection, root, 1), NULL);
    if (geometry == NULL || translated == NULL || monitors == NULL)
        goto done;

    iterator = xcb_randr_get_monitors_monitors_iterator(monitors);
    while (iterator.rem > 0) {
        const xcb_randr_monitor_info_t *monitor = iterator.data;
        int64_t area = overlap_area(
            translated->dst_x,
            translated->dst_y,
            translated->dst_x + geometry->width,
            translated->dst_y + geometry->height,
            monitor->x,
            monitor->y,
            monitor->x + monitor->width,
            monitor->y + monitor->height);

        if (area > best_area) {
            best_area = area;
            output.width = monitor->width;
            output.height = monitor->height;
            output.source = "randr_monitor";
        }
        xcb_randr_monitor_info_next(&iterator);
    }

done:
    free(monitors);
    free(translated);
    free(geometry);
    return output;
}

static void print_vendor(const xcb_setup_t *setup)
{
    const char *vendor = xcb_setup_vendor(setup);
    int length = xcb_setup_vendor_length(setup);
    int index;

    fputs("xserver_present_environment schema=1 vendor=", stdout);
    for (index = 0; index < length; index++) {
        unsigned char byte = (unsigned char)vendor[index];
        if ((byte >= 'a' && byte <= 'z') ||
            (byte >= 'A' && byte <= 'Z') ||
            (byte >= '0' && byte <= '9') ||
            byte == '.' || byte == '-' || byte == '_')
            putchar(byte);
        else
            putchar('_');
    }
}

static int parse_positive_u16(const char *text, uint16_t *value)
{
    char *end;
    unsigned long parsed;

    errno = 0;
    parsed = strtoul(text, &end, 10);
    if (errno != 0 || *text == '\0' || *end != '\0' ||
        parsed == 0 || parsed > UINT16_MAX)
        return -1;
    *value = (uint16_t)parsed;
    return 0;
}

static int parse_positive_u32(const char *text, uint32_t *value)
{
    char *end;
    unsigned long parsed;

    errno = 0;
    parsed = strtoul(text, &end, 10);
    if (errno != 0 || *text == '\0' || *end != '\0' ||
        parsed == 0 || parsed > UINT32_MAX)
        return -1;
    *value = (uint32_t)parsed;
    return 0;
}

int main(int argc, char **argv)
{
    uint16_t expected_width;
    uint16_t expected_height;
    uint32_t timeout_seconds;
    xcb_connection_t *connection;
    const xcb_setup_t *setup;
    xcb_screen_iterator_t screen_iterator;
    xcb_screen_t *screen;
    const xcb_query_extension_reply_t *present_extension;
    xcb_present_query_version_reply_t *present_version;
    xcb_generic_error_t *error;
    uint32_t root_event_mask = XCB_EVENT_MASK_SUBSTRUCTURE_NOTIFY;
    struct target_window targets[MAX_TARGET_WINDOWS];
    size_t target_count = 0;
    struct output_geometry output = {0};
    pid_t child;
    int child_status = 0;
    int child_finished = 0;
    int timed_out = 0;
    int kill_sent = 0;
    uint64_t deadline;
    uint64_t termination_deadline = 0;
    uint64_t drain_deadline = 0;
    uint32_t completion_count = 0;
    uint32_t flip_count = 0;
    uint32_t copy_count = 0;
    uint32_t skip_count = 0;

    if (argc < 6 || strcmp(argv[4], "--") != 0 ||
        parse_positive_u16(argv[1], &expected_width) != 0 ||
        parse_positive_u16(argv[2], &expected_height) != 0 ||
        parse_positive_u32(argv[3], &timeout_seconds) != 0) {
        fprintf(
            stderr,
            "usage: %s WIDTH HEIGHT TIMEOUT_SECONDS -- COMMAND [ARG ...]\n",
            argv[0]);
        return 2;
    }

    setvbuf(stdout, NULL, _IOLBF, 0);
    connection = xcb_connect(NULL, NULL);
    if (connection == NULL || xcb_connection_has_error(connection) != 0) {
        fprintf(stderr, "xserver Present probe could not connect to DISPLAY\n");
        return 1;
    }
    setup = xcb_get_setup(connection);
    screen_iterator = xcb_setup_roots_iterator(setup);
    screen = screen_iterator.data;
    if (screen == NULL) {
        fprintf(stderr, "xserver Present probe found no default screen\n");
        xcb_disconnect(connection);
        return 1;
    }

    present_extension = xcb_get_extension_data(connection, &xcb_present_id);
    if (present_extension == NULL || !present_extension->present) {
        fprintf(stderr, "xserver does not advertise the Present extension\n");
        xcb_disconnect(connection);
        return 1;
    }
    error = NULL;
    present_version = xcb_present_query_version_reply(
        connection,
        xcb_present_query_version(connection, 1, 2),
        &error);
    if (error != NULL || present_version == NULL) {
        fprintf(stderr, "xserver Present version query failed\n");
        free(error);
        free(present_version);
        xcb_disconnect(connection);
        return 1;
    }

    print_vendor(setup);
    printf(
        " root_width=%u root_height=%u present_major=%u present_minor=%u\n",
        screen->width_in_pixels,
        screen->height_in_pixels,
        present_version->major_version,
        present_version->minor_version);
    free(present_version);

    error = xcb_request_check(
        connection,
        xcb_change_window_attributes_checked(
            connection, screen->root, XCB_CW_EVENT_MASK, &root_event_mask));
    if (error != NULL) {
        fprintf(
            stderr,
            "xserver Present probe could not observe root children: error=%u\n",
            error->error_code);
        free(error);
        xcb_disconnect(connection);
        return 1;
    }
    xcb_flush(connection);

    child = fork();
    if (child < 0) {
        perror("fork");
        xcb_disconnect(connection);
        return 1;
    }
    if (child == 0) {
        execvp(argv[5], &argv[5]);
        perror("execvp");
        _exit(127);
    }

    deadline = monotonic_msec() + (uint64_t)timeout_seconds * 1000;
    while (!child_finished || monotonic_msec() < drain_deadline) {
        struct pollfd descriptor = {
            .fd = xcb_get_file_descriptor(connection),
            .events = POLLIN,
        };
        xcb_generic_event_t *event;
        int wait_result;
        pid_t wait_pid;

        if (!child_finished && !timed_out && monotonic_msec() >= deadline) {
            timed_out = 1;
            kill(child, SIGTERM);
            termination_deadline = monotonic_msec() + 1000;
        } else if (
            !child_finished && timed_out && !kill_sent &&
            monotonic_msec() >= termination_deadline) {
            kill_sent = 1;
            kill(child, SIGKILL);
        }

        wait_result = poll(&descriptor, 1, CHILD_POLL_MSEC);
        if (wait_result < 0 && errno != EINTR) {
            perror("poll");
            kill(child, SIGTERM);
            waitpid(child, &child_status, 0);
            xcb_disconnect(connection);
            return 1;
        }

        while ((event = xcb_poll_for_event(connection)) != NULL) {
            uint8_t response_type = event->response_type & 0x7f;

            if (response_type == XCB_CREATE_NOTIFY) {
                const xcb_create_notify_event_t *created =
                    (const xcb_create_notify_event_t *)event;
                if (created->parent == screen->root &&
                    created->width == expected_width &&
                    created->height == expected_height &&
                    attach_target(
                        connection,
                        targets,
                        &target_count,
                        created->window) != 0) {
                    free(event);
                    kill(child, SIGTERM);
                    waitpid(child, &child_status, 0);
                    xcb_disconnect(connection);
                    return 1;
                }
                xcb_flush(connection);
            } else if (
                response_type == XCB_GE_GENERIC &&
                ((xcb_ge_generic_event_t *)event)->extension ==
                    present_extension->major_opcode) {
                const xcb_present_complete_notify_event_t *complete =
                    (const xcb_present_complete_notify_event_t *)event;
                if (complete->event_type == XCB_PRESENT_COMPLETE_NOTIFY &&
                    complete->kind == XCB_PRESENT_COMPLETE_KIND_PIXMAP &&
                    target_index(targets, target_count, complete->window) >= 0) {
                    if (output.width == 0)
                        output = output_for_window(
                            connection, screen->root, complete->window, screen);
                    completion_count++;
                    if (complete->mode == XCB_PRESENT_COMPLETE_MODE_FLIP)
                        flip_count++;
                    else if (
                        complete->mode == XCB_PRESENT_COMPLETE_MODE_COPY ||
                        complete->mode ==
                            XCB_PRESENT_COMPLETE_MODE_SUBOPTIMAL_COPY)
                        copy_count++;
                    else if (complete->mode == XCB_PRESENT_COMPLETE_MODE_SKIP)
                        skip_count++;
                    printf(
                        "xserver_present_feedback schema=1 kind=complete "
                        "mode=%s ust=%" PRIu64 " msc=%" PRIu64 "\n",
                        present_mode_name(complete->mode),
                        complete->ust,
                        complete->msc);
                }
            }
            free(event);
        }

        if (!child_finished) {
            wait_pid = waitpid(child, &child_status, WNOHANG);
            if (wait_pid == child) {
                child_finished = 1;
                drain_deadline = monotonic_msec() + DRAIN_MSEC;
            } else if (wait_pid < 0) {
                perror("waitpid");
                xcb_disconnect(connection);
                return 1;
            }
        }
    }

    if (output.width == 0) {
        output.width = screen->width_in_pixels;
        output.height = screen->height_in_pixels;
        output.source = "root";
    }
    printf(
        "xserver_present_probe schema=1 status=%s child_status=%d "
        "target_windows=%zu present_completions=%u flips=%u copies=%u "
        "skips=%u output_width=%u output_height=%u output_source=%s\n",
        !timed_out && WIFEXITED(child_status) && WEXITSTATUS(child_status) == 0 &&
                completion_count >= 3
            ? "complete"
            : "failed",
        WIFEXITED(child_status) ? WEXITSTATUS(child_status) : 128,
        target_count,
        completion_count,
        flip_count,
        copy_count,
        skip_count,
        output.width,
        output.height,
        output.source);
    xcb_disconnect(connection);

    return !timed_out && WIFEXITED(child_status) &&
                   WEXITSTATUS(child_status) == 0 && completion_count >= 3
               ? 0
               : 1;
}
