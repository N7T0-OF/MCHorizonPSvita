#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// Vanilla's `Mth.floor`, using a saturating float-to-int conversion and a
/// correction for negative fractions. Shared by desktop simulation and the
/// no_std noise implementation.
#[inline]
pub(crate) fn floor_i32(value: f64) -> i32 {
    let truncated = value as i32;
    if value < f64::from(truncated) {
        truncated.saturating_sub(1)
    } else {
        truncated
    }
}

#[inline]
pub(crate) fn floor_f64(value: f64) -> f64 {
    #[cfg(feature = "wiiu-core")]
    {
        libm::floor(value)
    }
    #[cfg(not(feature = "wiiu-core"))]
    {
        value.floor()
    }
}

#[inline]
pub(crate) fn sqrt_f64(value: f64) -> f64 {
    #[cfg(feature = "wiiu-core")]
    {
        libm::sqrt(value)
    }
    #[cfg(not(feature = "wiiu-core"))]
    {
        value.sqrt()
    }
}

#[inline]
pub(crate) fn log_f64(value: f64) -> f64 {
    #[cfg(feature = "wiiu-core")]
    {
        libm::log(value)
    }
    #[cfg(not(feature = "wiiu-core"))]
    {
        crate::jmath::log(value)
    }
}
