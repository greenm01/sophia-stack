use super::*;

impl PersistentXtermSessionConfig {
    pub(super) fn surface_chrome_style(
        style: sophia_config::ChromePolicy,
    ) -> sophia_engine::SurfaceChromeStyle {
        sophia_engine::SurfaceChromeStyle {
            focus_ring: sophia_engine::FocusRingStyle {
                width: if style.focus_ring.enabled {
                    i32::try_from(style.focus_ring.width).unwrap_or(i32::MAX)
                } else {
                    0
                },
                color: config_rgb(style.focus_ring.color),
            },
            frame: sophia_engine::SurfaceFrameStyle {
                width: if style.frame.enabled {
                    i32::try_from(style.frame.width).unwrap_or(i32::MAX)
                } else {
                    0
                },
                focused_color: config_rgb(style.frame.focused_color),
                unfocused_color: config_rgb(style.frame.unfocused_color),
            },
        }
    }

    pub(super) fn wm_surface_chrome_style(
        style: sophia_protocol::WmChromePolicy,
    ) -> sophia_engine::SurfaceChromeStyle {
        sophia_engine::SurfaceChromeStyle {
            focus_ring: sophia_engine::FocusRingStyle {
                width: if style.focus_ring.enabled {
                    i32::try_from(style.focus_ring.width).unwrap_or(i32::MAX)
                } else {
                    0
                },
                color: wm_rgb(style.focus_ring.color),
            },
            frame: sophia_engine::SurfaceFrameStyle {
                width: if style.frame.enabled {
                    i32::try_from(style.frame.width).unwrap_or(i32::MAX)
                } else {
                    0
                },
                focused_color: wm_rgb(style.frame.focused_color),
                unfocused_color: wm_rgb(style.frame.unfocused_color),
            },
        }
    }
}

fn config_rgb(color: sophia_config::Rgb8) -> sophia_engine::CompositorRgb8 {
    sophia_engine::CompositorRgb8 {
        red: color.red,
        green: color.green,
        blue: color.blue,
    }
}

fn wm_rgb(color: sophia_protocol::WmRgb8) -> sophia_engine::CompositorRgb8 {
    sophia_engine::CompositorRgb8 {
        red: color.red,
        green: color.green,
        blue: color.blue,
    }
}
