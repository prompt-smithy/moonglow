//! Pure routing calculations for dual hardware and software brightness.

use crate::settings::{AppSettings, SOFTWARE_BRIGHTNESS_FLOOR};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingOutput {
    pub hardware: Option<i32>,
    pub software: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitOutput {
    pub hardware: i32,
    pub software: i32,
}

pub fn compute_routing(
    master: i32,
    split: bool,
    manual_hardware: i32,
    manual_software: i32,
    hardware_available: bool,
    software_available: bool,
    recovery_registered: bool,
) -> RoutingOutput {
    let floor = if recovery_registered {
        1
    } else {
        SOFTWARE_BRIGHTNESS_FLOOR
    };

    if split {
        let hw = if hardware_available {
            Some(AppSettings::clamp_brightness(manual_hardware))
        } else {
            None
        };
        let sw = if software_available {
            AppSettings::clamp_brightness(manual_software).max(floor)
        } else {
            100
        };
        return RoutingOutput {
            hardware: hw,
            software: sw,
        };
    }

    let clamped_master = AppSettings::clamp_brightness(master);
    if hardware_available && software_available {
        if clamped_master >= 50 {
            let hw = 1 + ((clamped_master - 50) as f64 * 99.0 / 50.0).round() as i32;
            RoutingOutput {
                hardware: Some(hw),
                software: 100,
            }
        } else {
            let sw = if floor == 1 {
                1 + ((clamped_master - 1) as f64 * 99.0 / 49.0).round() as i32
            } else {
                50 + ((clamped_master - 1) as f64 * 49.0 / 48.0).round() as i32
            };
            RoutingOutput {
                hardware: Some(1),
                software: sw,
            }
        }
    } else if hardware_available {
        RoutingOutput {
            hardware: Some(clamped_master),
            software: 100,
        }
    } else if software_available {
        RoutingOutput {
            hardware: None,
            software: clamped_master.max(floor),
        }
    } else {
        RoutingOutput {
            hardware: None,
            software: 100,
        }
    }
}

pub fn unified_to_split(
    master: i32,
    hardware_available: bool,
    software_available: bool,
    recovery_registered: bool,
) -> SplitOutput {
    let routing = compute_routing(
        master,
        false,
        80,
        100,
        hardware_available,
        software_available,
        recovery_registered,
    );
    SplitOutput {
        hardware: routing.hardware.unwrap_or(80),
        software: routing.software,
    }
}

pub fn split_to_unified(
    hardware: i32,
    software: i32,
    hardware_available: bool,
    software_available: bool,
    recovery_registered: bool,
) -> i32 {
    if hardware_available && software_available {
        let hw = AppSettings::clamp_brightness(hardware);
        let sw = AppSettings::clamp_brightness(software).max(if recovery_registered {
            1
        } else {
            SOFTWARE_BRIGHTNESS_FLOOR
        });
        let effective = (hw * sw) as f64 / 100.0;
        if effective > 1.0 {
            50 + ((effective - 1.0) * 50.0 / 99.0).round() as i32
        } else {
            let shade = ((effective * 100.0).round() as i32).max(if recovery_registered {
                1
            } else {
                SOFTWARE_BRIGHTNESS_FLOOR
            });
            if shade >= 100 {
                50
            } else if recovery_registered {
                1 + ((shade - 1) as f64 * 49.0 / 99.0).round() as i32
            } else {
                1 + ((shade - 50) as f64 * 48.0 / 49.0).round() as i32
            }
        }
    } else if hardware_available {
        AppSettings::clamp_brightness(hardware)
    } else if software_available {
        AppSettings::clamp_brightness(software)
    } else {
        80
    }
}

/// Re-expresses the applied light level under a new engine set or software floor.
pub fn master_preserving_output(
    output: RoutingOutput,
    hardware_available: bool,
    software_available: bool,
    recovery_registered: bool,
) -> i32 {
    let hardware = output.hardware.unwrap_or(100);
    if hardware_available && software_available {
        return split_to_unified(hardware, output.software, true, true, recovery_registered);
    }
    let effective = ((hardware * output.software) as f64 / 100.0).round() as i32;
    AppSettings::clamp_brightness(effective).max(if software_available && !recovery_registered {
        SOFTWARE_BRIGHTNESS_FLOOR
    } else {
        1
    })
}
