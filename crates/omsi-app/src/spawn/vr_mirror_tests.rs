use super::*;
use crate::vr_mirrors::Plan;
use glam::Mat4;
use omsi_geometry::MeshData;
use omsi_render::{AlphaMode, Camera, RenderOptions, Renderer};

fn player() -> Player {
    let mut p = super::cabin_input_tests::player();
    let def = &mut Arc::make_mut(&mut p.vehicle.ty).def;
    def.cameras_driver = vec![omsi_vehicle::Camera {
        pos: [0.0, 0.0, 2.0],
        ..Default::default()
    }];
    def.cameras_reflexion = vec![
        omsi_vehicle::Camera {
            pos: [1.0, 3.0, 2.0],
            yaw: 180.0,
            fov: 50.0,
            ..Default::default()
        },
        omsi_vehicle::Camera {
            pos: [0.0, -6.0, 4.5],
            yaw: 90.0,
            pitch: -25.0,
            fixed: true,
            fov: 45.0,
            ..Default::default()
        },
        // Existing mods also use a non-static declaration for an upper-deck feed.
        omsi_vehicle::Camera {
            pos: [0.0, -6.0, 4.5],
            yaw: 180.0,
            pitch: -15.0,
            fov: 45.0,
            ..Default::default()
        },
    ];
    p
}

fn views(yaw: f32) -> [(Camera, Mat4); 2] {
    std::array::from_fn(|eye| {
        (
            Camera {
                position: DVec3::new(if eye == 0 { -0.032 } else { 0.032 }, 0.0, 2.0),
                yaw,
                pitch: 0.0,
                roll: 0.0,
                fov_deg: 70.0,
                near: 0.1,
                far: 1000.0,
            },
            Mat4::perspective_rh(70f32.to_radians(), 1.0, 1000.0, 0.1),
        )
    })
}

#[test]
fn vr_mirror_cameras_follow_the_vehicle_and_keep_coupled_camera_numbering() {
    let mut p = player();
    p.mirror_offsets = vec![[4.0, -3.0], [7.0, 2.0], [0.0; 2], [9.0, 1.0]];
    let mut ty = (*p.vehicle.ty).clone();
    ty.def.cameras_reflexion = vec![ty.def.cameras_reflexion[1].clone()];
    let mut part =
        omsi_sim::vehicle::TrailerPart::new(Arc::new(ty), &p.vehicle.ty, &p.vehicle.ty.program, 0);
    part.position = DVec3::new(3.0, -8.0, 0.0);
    part.heading = 0.4;
    p.vehicle.trailers.push(part);
    let build =
        |p: &Player| crate::camera_util::mirror_cameras(p, crate::camera_util::driver_eye(p));
    let before = build(&p);
    assert_eq!(before.len(), 4);
    assert_eq!((before[1].0.yaw, before[1].0.pitch), (97.0, -23.0));
    assert_eq!((before[3].0.yaw, before[3].0.pitch), (99.0, -24.0));
    p.vehicle.position = DVec3::new(1000.0, -500.0, 3.0);
    p.vehicle.heading = 0.8;
    p.vehicle.pitch = 0.1;
    let after = build(&p);
    let camera = &after[0].0;
    let expected = p.vehicle.position
        + p.vehicle
            .body_rotation()
            .transform_point3(Vec3::from_array(camera.pos))
            .as_dvec3();
    assert!((after[0].1.0 - expected).length() < 1e-6);
    assert!((after[0].0.yaw - before[0].0.yaw).abs() < 1e-3);
    assert!((after[0].0.pitch - before[0].0.pitch).abs() < 1e-3);
    assert_eq!(after[3], before[3]);
}

#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn vr_mirror_surfaces_use_the_monitor_not_the_remote_camera_and_keep_aim_stable() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let renderer = pollster::block_on(Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        RenderOptions {
            msaa: 1,
            ssao: false,
            ray_tracing: false,
            shadow_size: 1024,
            ..Default::default()
        },
    ))
    .expect("test renderer");
    let mut scene = renderer.new_scene();
    let textures: Vec<_> = (0..3)
        .map(|_| Some(renderer.add_render_texture(&mut scene, 16, 16)))
        .collect();
    let mesh = renderer.add_mesh(
        &mut scene,
        &MeshData {
            positions: vec![
                Vec3::new(-0.2, 0.0, -0.2),
                Vec3::new(0.2, 0.0, -0.2),
                Vec3::new(0.0, 0.0, 0.2),
            ],
            normals: vec![-Vec3::Y; 3],
            uvs: vec![glam::Vec2::ZERO; 3],
            indices: vec![0, 1, 2],
            ranges: vec![(0, 3, 0)],
            one_sided: false,
        },
    );
    let mut p = player();
    for (i, &texture) in textures.iter().enumerate() {
        let material =
            renderer.add_material(&mut scene, texture, AlphaMode::Opaque, [1.0; 4], true);
        let y = if i == 0 { -3.0 } else { 3.0 };
        let id = renderer.add_instance(
            &mut scene,
            mesh,
            DVec3::new(0.0, y, 2.0),
            Mat4::IDENTITY,
            vec![material],
        );
        p.render.instances.push(id);
    }
    let a = Plan::new(&p, &scene, &textures, Some(&views(0.0)));
    assert_eq!(
        a.visible,
        vec![1, 2],
        "the visible upper-deck monitor must stay live"
    );
    let b = Plan::new(&p, &scene, &textures, Some(&views(180.0)));
    assert_eq!(b.visible, vec![0, 1]);
    assert_eq!(
        a.cameras, b.cameras,
        "head rotation must not re-aim any camera"
    );
    let mut shifted = views(35.0);
    for (camera, projection) in &mut shifted {
        camera.position += DVec3::new(-0.3, 0.15, 0.1);
        projection.z_axis.x = 0.2;
    }
    assert_eq!(
        a.cameras,
        Plan::new(&p, &scene, &textures, Some(&shifted)).cameras
    );
    // No history cache: an animation moving the display is visible immediately.
    renderer.set_transform(
        &mut scene,
        p.render.instances[2],
        DVec3::new(0.0, -3.0, 2.0),
        Mat4::IDENTITY,
    );
    assert_eq!(
        Plan::new(&p, &scene, &textures, Some(&views(0.0))).visible,
        vec![1]
    );
    assert_eq!(
        Plan::new(&p, &scene, &[], Some(&views(0.0))).visible,
        vec![0, 1, 2]
    );
    assert_eq!(
        Plan::new(&p, &scene, &textures, None).visible,
        vec![0, 1, 2]
    );
    assert_eq!(
        Plan::new(
            &p,
            &scene,
            &textures,
            Some(&[(views(0.0)[0].0, Mat4::ZERO); 2])
        )
        .visible,
        vec![0, 1, 2]
    );
    // A consumer on a coupled part counts too, even if the lead body's copy is hidden.
    scene.instances[p.render.instances[2]].visible = false;
    let mut rear = super::cabin_input_tests::player().render;
    let material =
        renderer.add_material(&mut scene, textures[2], AlphaMode::Opaque, [1.0; 4], true);
    rear.instances.push(renderer.add_instance(
        &mut scene,
        mesh,
        DVec3::new(0.0, 3.0, 2.0),
        Mat4::IDENTITY,
        vec![material],
    ));
    p.trailer_renders.push(rear);
    assert_eq!(
        Plan::new(&p, &scene, &textures, Some(&views(0.0))).visible,
        vec![1, 2]
    );
}
