# Stereo frame preparation

`Renderer::render_xr_stereo` draws both OpenXR eyes from one scene snapshot.
It applies pending instance/bounds uploads once, then traverses the scene's
bounding blocks once to collect candidates for both asymmetric eye frusta.
Object origins and radii are projected once per object per eye, rather than
once per mesh. Smaller culling leaves retain the same instance budget for
tightening dirty bounds; growing bounds remain conservative immediately.

The shared data lives only for this call. Moving or reshaping a mesh, recycling
an instance, or streaming a tile cannot leave a candidate cache from a previous
frame. Widely separated views that would require a render-origin change between
eyes use the existing independent preparation path.

Each eye retains its own visibility bits, object-size/LOD hysteresis and cached
render bundles. Final culling, draw-list offsets, camera uniforms and transparent
sorting remain independent. The desktop's history is restored after each eye.
The existing shared shadow atlas and D3D12/OpenXR queue ordering are preserved.
The first eye's reported CPU duration includes shared preparation.

## Traffic mesh detail

Shared AI vehicle instances opt into a one-pixel mesh cutoff in headset views.
The projected bounding sphere uses its nearest point and the eye's actual
projection and render height. A previously visible mesh has a 15% deadband.
This removes subpixel fittings without changing the authored whole-vehicle LOD
selection, player vehicle detail, simulation, desktop rendering or mirror detail.
Unlit, emissive and night-mapped meshes remain visible for their light and glow.
Pooled instances receive the policy after slot acquisition, including an explicit
reset for player instances. This is not a general AI model simplifier. Upstream's authored vehicle LOD loader and bands remain unchanged; both near and far instances receive the headset mesh policy, and final LOD selection stays per eye.

## Mirror refresh

VR refresh selection uses both last-submitted eye projections. Asymmetric FOVs
are widened conservatively by 15 degrees and mirror bounds by 35 cm to allow for
the age of the pose. Fixed cameras continue refreshing because their monitors
need not be positioned at the camera itself. Coupled vehicle cameras participate
in the same numbering and budget as their textures.

All mirror images initialize on a bus or XR session change. Newly visible
mirrors refresh immediately, even if the normal budget has not accrued a turn.
Regular visible turns and at most one aggregate hidden refresh per second use
the selected total redraw budget. A zero rate freezes existing images; the
separate Off setting retains its existing behavior. Desktop mirror scheduling
is unchanged. This uses a previous-frame pose, not late-latched visibility.

## Regression coverage

The renderer's explicit GPU tests compare paired rendering with independent
eye calls, including per-eye image readback, LOD/distance histories, scene and
origin changes, bundle ownership and AI mesh detail. These run without a headset.
Pure tests cover asymmetric mirror visibility, initialization, newly visible
mirrors, aggregate background refresh, redraw limits and zero-rate behavior.

The changes reduce repeated preparation and unnecessary work. They do not imply
a particular headset frame rate or establish the cause of a compositor error.
