use input::{AccelProfile, Device, DeviceCapability};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeLibinputAccelProfile {
    Flat,
    Adaptive,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeLibinputPointerPolicy {
    pub natural_scroll: Option<bool>,
    pub accel_profile: Option<NativeLibinputAccelProfile>,
    pub accel_speed: Option<f64>,
    pub left_handed: Option<bool>,
    pub middle_emulation: Option<bool>,
    pub scroll_factor: f64,
}

impl Default for NativeLibinputPointerPolicy {
    fn default() -> Self {
        Self {
            natural_scroll: None,
            accel_profile: None,
            accel_speed: None,
            left_handed: None,
            middle_emulation: None,
            scroll_factor: 1.0,
        }
    }
}

impl NativeLibinputPointerPolicy {
    pub fn validate(self) -> Option<Self> {
        let speed_valid = self
            .accel_speed
            .is_none_or(|speed| speed.is_finite() && (-1.0..=1.0).contains(&speed));
        let factor_valid =
            self.scroll_factor.is_finite() && (0.01..=10.0).contains(&self.scroll_factor);
        (speed_valid && factor_valid).then_some(self)
    }

    pub const fn requires_device_configuration(self) -> bool {
        self.natural_scroll.is_some()
            || self.accel_profile.is_some()
            || self.accel_speed.is_some()
            || self.left_handed.is_some()
            || self.middle_emulation.is_some()
    }

    pub fn scale_scroll_v120(self, value: f64) -> i32 {
        (value * self.scroll_factor)
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
    }
}

pub(crate) fn apply_native_pointer_policy(
    device: &mut Device,
    policy: NativeLibinputPointerPolicy,
) -> bool {
    if !device.has_capability(DeviceCapability::Pointer) {
        return true;
    }
    if let Some(enabled) = policy.natural_scroll
        && device.config_scroll_natural_scroll_enabled() != enabled
        && device
            .config_scroll_set_natural_scroll_enabled(enabled)
            .is_err()
    {
        return false;
    }
    if let Some(profile) = policy.accel_profile {
        let profile = match profile {
            NativeLibinputAccelProfile::Flat => AccelProfile::Flat,
            NativeLibinputAccelProfile::Adaptive => AccelProfile::Adaptive,
        };
        if device.config_accel_profile() != Some(profile)
            && device.config_accel_set_profile(profile).is_err()
        {
            return false;
        }
    }
    if let Some(speed) = policy.accel_speed
        && (device.config_accel_speed() - speed).abs() > f64::EPSILON
        && device.config_accel_set_speed(speed).is_err()
    {
        return false;
    }
    if let Some(enabled) = policy.left_handed
        && device.config_left_handed() != enabled
        && device.config_left_handed_set(enabled).is_err()
    {
        return false;
    }
    if let Some(enabled) = policy.middle_emulation
        && device.config_middle_emulation_enabled() != enabled
        && device.config_middle_emulation_set_enabled(enabled).is_err()
    {
        return false;
    }
    true
}
