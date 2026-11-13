#![cfg(feature = "rust-ui")]
use std::time::Duration;
use volt_ui_kit::ui::prelude::*;
#[test]
fn transforms_roundtrip_at_fractional_dpi_and_compose_in_order() {
    let child = Affine2D([2., 0., 0., 3., 4., 5.]);
    let parent = Affine2D([0., 1., -1., 0., 9., 10.]);
    assert_eq!(
        child.then(parent).point([2., 3.]),
        parent.point(child.point([2., 3.]))
    );
    for dpi in [1., 1.25, 1.5, 2., 3.] {
        let a = Transform2D::new()
            .translate(12.5, -3.)
            .rotate(37.)
            .scale(1.4, 0.8);
        let m = a.matrix([40., 25., 100., 50.], dpi);
        let p = [73.3 * dpi, 42.7 * dpi];
        let q = m.inverse().unwrap().point(m.point(p));
        assert!((q[0] - p[0]).abs() < 0.001 && (q[1] - p[1]).abs() < 0.001);
    }
}
#[test]
fn singular_transform_is_not_hittable_and_nonfinite_values_are_rejected() {
    assert!(Transform2D::new()
        .scale(0., 1.)
        .matrix([0., 0., 100., 100.], 1.)
        .inverse()
        .is_none());
    assert!(!Transform2D::new().rotate(f32::NAN).is_valid());
    assert!(Affine2D([1e300, 0., 0., 1e300, 0., 0.]).inverse().is_none());
}
#[test]
fn timeline_seek_pause_reverse_and_finish_are_deterministic() {
    let timeline = Timeline::new(1000)
        .keyframe(0., MotionPose::new())
        .keyframe(1., MotionPose::new().translate(100., 0.).opacity(0.));
    let mut player = TimelinePlayer::new(timeline).unwrap();
    player.play(Duration::ZERO);
    let middle = player.sample(Duration::from_millis(500));
    assert!((middle.translation[0] - 50.).abs() < 0.001);
    player.pause(Duration::from_millis(500));
    assert_eq!(player.sample(Duration::from_secs(20)), middle);
    player.reverse(Duration::from_secs(20));
    assert_eq!(
        player.sample(Duration::from_millis(20500)),
        MotionPose::new()
    );
    assert!(!player.is_playing());
    player.seek(1.);
    assert_eq!(player.sample(Duration::from_secs(40)).opacity, 0.);
}
#[test]
fn inertia_is_frame_rate_independent_and_bounded() {
    let config = Inertia::new().bounds([-100., -50.], [100., 50.]);
    let mut a = config.release([0., 0.], [800., 300.]).unwrap();
    let mut b = a.clone();
    for t in (0..=2000).step_by(8) {
        a.sample(Duration::from_millis(t));
    }
    let pa = a.sample(Duration::from_secs(2));
    let pb = b.sample(Duration::from_secs(2));
    assert_eq!(pa, pb);
    assert!(pa[0] <= 100. && pa[1] <= 50.);
    assert!(!a.is_active());
}
