use super::*;

fn seat() -> Camera {
    Camera {
        position: DVec3::new(100_000.0, 200_000.0, 1.8),
        yaw: 90.0,
        pitch: -20.0,
        roll: 5.0,
        fov_deg: 90.0,
        near: 0.05,
        far: 1000.0,
    }
}

fn fov() -> xr::Fovf {
    xr::Fovf {
        angle_left: -0.8,
        angle_right: 0.8,
        angle_up: 0.8,
        angle_down: -0.8,
    }
}

#[test]
fn menu_opens_half_a_metre_ahead_of_the_level_seat() {
    let seat = seat();
    let mut anchor = None;
    UiAnchor::update_menu(&mut anchor, true, &seat, fov(), (1920, 1080));
    let menu = anchor.unwrap();
    assert!((menu.origin - (seat.position + DVec3::X * 0.5)).length() < 1e-6);
    assert!((menu.up - Vec3::Z).length() < 1e-6);
    assert!((menu.half_width / menu.half_height - 1920.0 / 1080.0).abs() < 1e-6);

    // Looking left at the instant of opening changes the projection, never
    // the anchor: no tracked eye pose is an input to menu placement.
    let ahead = Camera {
        pitch: 0.0,
        roll: 0.0,
        ..seat
    };
    let left = Camera { yaw: 0.0, ..ahead };
    let projection = eye_projection(fov(), 0.05, 1000.0);
    let centre = menu.cursor_clip(&ahead, projection);
    assert!((centre.x / centre.w).abs() < 1e-5);
    let side = menu.cursor_clip(&left, projection);
    assert!(side.x.abs() > 0.4);
}

#[test]
fn menu_keeps_its_world_pose_until_closed_and_reopens_at_the_current_seat() {
    let seat = seat();
    let mut anchor = None;
    UiAnchor::update_menu(&mut anchor, true, &seat, fov(), (1920, 1080));
    let first = anchor.unwrap();
    let moved = Camera {
        position: seat.position + DVec3::Y * 10.0,
        yaw: 180.0,
        ..seat
    };
    UiAnchor::update_menu(&mut anchor, true, &moved, fov(), (1280, 1024));
    let retained = anchor.unwrap();
    assert_eq!(retained.origin, first.origin);
    assert_eq!(retained.right, first.right);
    assert_eq!(retained.up, first.up);
    assert_eq!(retained.half_height, first.half_height);
    assert!((retained.half_width / retained.half_height - 1.25).abs() < 1e-6);
    UiAnchor::update_menu(&mut anchor, false, &moved, fov(), (1280, 1024));
    assert!(anchor.is_none());
    UiAnchor::update_menu(&mut anchor, true, &moved, fov(), (1280, 1024));
    assert!((anchor.unwrap().origin - (moved.position - DVec3::Y * 0.5)).length() < 1e-6);
}
