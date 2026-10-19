/* Headless baseline using the pinned SDK's public C API. */
#include <libobsensor/ObSensor.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

static void check(ob_error **error) {
    if (*error) {
        fprintf(stderr, "%s: %s\n", ob_error_function(*error), ob_error_message(*error));
        ob_delete_error(*error);
        *error = NULL;
        exit(EXIT_FAILURE);
    }
}
#define CALL(expr) do { expr; check(&error); } while (0)

int main(void) {
    ob_error *error = NULL;
    printf("C baseline SDK %d.%d.%d\n", ob_get_major_version(), ob_get_minor_version(), ob_get_patch_version());
    CALL(ob_set_logger_to_console(OB_LOG_SEVERITY_WARN, &error));
    ob_context *context = ob_create_context(&error); check(&error);
    ob_device_list *devices = ob_query_device_list(context, &error); check(&error);
    uint32_t count = ob_device_list_device_count(devices, &error); check(&error);
    printf("Devices: %u\n", count);
    if (!count) { fprintf(stderr, "No camera detected; check USB permissions\n"); return 2; }
    ob_device *device = ob_device_list_get_device(devices, 0, &error); check(&error);
    ob_device_info *info = ob_device_get_device_info(device, &error); check(&error);
    const char *name = ob_device_info_name(info, &error); check(&error);
    const char *firmware = ob_device_info_firmware_version(info, &error); check(&error);
    printf("Camera: %s firmware=%s\n", name, firmware);
    ob_pipeline *pipeline = ob_create_pipeline_with_device(device, &error); check(&error);
    ob_stream_profile_list *profiles = ob_pipeline_get_stream_profile_list(pipeline, OB_SENSOR_DEPTH, &error); check(&error);
    uint32_t profile_count = ob_stream_profile_list_count(profiles, &error); check(&error);
    printf("Depth profiles: %u\n", profile_count);
    int selected = 0;
    for (uint32_t i = 0; i < profile_count; ++i) {
        ob_stream_profile *p = ob_stream_profile_list_get_profile(profiles, (int)i, &error); check(&error);
        uint32_t width = ob_video_stream_profile_width(p, &error); check(&error);
        uint32_t height = ob_video_stream_profile_height(p, &error); check(&error);
        uint32_t fps = ob_video_stream_profile_fps(p, &error); check(&error);
        ob_format format = ob_stream_profile_format(p, &error); check(&error);
        printf("  [%u] %ux%u @%u format=%d\n", i, width, height, fps, format);
        if (format == OB_FORMAT_Y16 && width == 640 && fps == 30) selected = (int)i;
        CALL(ob_delete_stream_profile(p, &error));
    }
    ob_stream_profile *profile = ob_stream_profile_list_get_profile(profiles, selected, &error); check(&error);
    ob_config *config = ob_create_config(&error); check(&error);
    CALL(ob_config_disable_all_stream(config, &error));
    CALL(ob_config_enable_stream(config, profile, &error));
    CALL(ob_pipeline_start_with_config(pipeline, config, &error));
    unsigned captured = 0;
    for (unsigned attempts = 0; attempts < 40 && captured < 30; ++attempts) {
        ob_frame *frames = ob_pipeline_wait_for_frameset(pipeline, 1000, &error); check(&error);
        if (!frames) continue;
        ob_frame *depth = ob_frameset_depth_frame(frames, &error); check(&error);
        if (depth) {
            uint32_t width = ob_video_frame_width(depth, &error); check(&error);
            uint32_t height = ob_video_frame_height(depth, &error); check(&error);
            uint32_t bytes = ob_frame_data_size(depth, &error); check(&error);
            uint64_t stamp = ob_frame_time_stamp_us(depth, &error); check(&error);
            float scale = ob_depth_frame_get_value_scale(depth, &error); check(&error);
            if (!captured || captured == 29)
                printf("depth[%u] %ux%u bytes=%u timestamp_us=%" PRIu64 " scale_mm=%g\n", captured, width, height, bytes, stamp, scale);
            ++captured;
            CALL(ob_delete_frame(depth, &error));
        }
        CALL(ob_delete_frame(frames, &error));
    }
    CALL(ob_pipeline_stop(pipeline, &error));
    CALL(ob_delete_config(config, &error));
    CALL(ob_delete_stream_profile(profile, &error));
    CALL(ob_delete_stream_profile_list(profiles, &error));
    CALL(ob_delete_pipeline(pipeline, &error));
    CALL(ob_delete_device_info(info, &error));
    CALL(ob_delete_device(device, &error));
    CALL(ob_delete_device_list(devices, &error));
    CALL(ob_delete_context(context, &error));
    printf("Captured %u/30 depth frames; cleaned up\n", captured);
    return captured == 30 ? EXIT_SUCCESS : EXIT_FAILURE;
}
