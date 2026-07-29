#![cfg(feature = "gbm-platform")]

use sophia_renderer_native_egl::{
    NativeRendererImageCacheAdmission, NativeRendererImageId, native_renderer_image_cache_admission,
};

fn id(raw: u64) -> NativeRendererImageId {
    NativeRendererImageId::from_raw(raw)
}

#[test]
fn cache_admission_distinguishes_cold_hit_and_full_tables() {
    assert_eq!(
        native_renderer_image_cache_admission([None, None], id(1)),
        NativeRendererImageCacheAdmission::Vacant { slot: 0 }
    );
    assert_eq!(
        native_renderer_image_cache_admission([Some(id(1)), None], id(1)),
        NativeRendererImageCacheAdmission::Hit { slot: 0 }
    );
    assert_eq!(
        native_renderer_image_cache_admission([Some(id(1)), Some(id(2))], id(3)),
        NativeRendererImageCacheAdmission::Full
    );
}

#[test]
fn cache_admission_treats_each_generation_as_distinct() {
    assert_eq!(
        native_renderer_image_cache_admission([Some(id(7)), None], id(8)),
        NativeRendererImageCacheAdmission::Vacant { slot: 1 }
    );
    assert_eq!(
        native_renderer_image_cache_admission([Some(id(8)), None], id(7)),
        NativeRendererImageCacheAdmission::Vacant { slot: 1 }
    );
}

#[test]
fn cache_admission_rejects_the_invalid_generation() {
    assert_eq!(
        native_renderer_image_cache_admission([None], NativeRendererImageId::INVALID),
        NativeRendererImageCacheAdmission::Invalid
    );
}
