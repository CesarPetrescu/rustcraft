use cgmath::{perspective, vec3, InnerSpace, Matrix4, Point3, Rad, SquareMatrix, Vector3, Vector4};
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub const PLAYER_HEIGHT: f32 = 1.8;
pub const PLAYER_EYE_HEIGHT: f32 = 1.62;
pub const PLAYER_RADIUS: f32 = 0.3;

const GRAVITY: f32 = -25.0;
const JUMP_VELOCITY: f32 = 8.0;

pub struct Camera {
    pub position: Point3<f32>,
    pub yaw: Rad<f32>,
    pub pitch: Rad<f32>,
}

impl Camera {
    pub fn new(position: Point3<f32>, yaw: Rad<f32>, pitch: Rad<f32>) -> Self {
        Self {
            position,
            yaw,
            pitch,
        }
    }

    pub fn calc_matrix(&self, projection: &Projection) -> Matrix4<f32> {
        projection.build_matrix() * Matrix4::look_to_rh(self.position, self.direction(), Self::UP)
    }

    pub fn direction(&self) -> Vector3<f32> {
        let (sin_yaw, cos_yaw) = self.yaw.0.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.0.sin_cos();

        vec3(cos_pitch * cos_yaw, sin_pitch, cos_pitch * sin_yaw).normalize()
    }

    pub fn right(&self) -> Vector3<f32> {
        self.direction().cross(Self::UP).normalize()
    }

    const UP: Vector3<f32> = vec3(0.0, 1.0, 0.0);
}

pub struct Projection {
    aspect: f32,
    fov_y: Rad<f32>,
    base_fov: Rad<f32>,
    target_fov: Rad<f32>,
    znear: f32,
    zfar: f32,
}

impl Projection {
    pub fn new(width: u32, height: u32, fov_y: f32, znear: f32, zfar: f32) -> Self {
        let fov_rad = Rad(fov_y);
        Self {
            aspect: width as f32 / height as f32,
            fov_y: fov_rad,
            base_fov: fov_rad,
            target_fov: fov_rad,
            znear,
            zfar,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    pub fn aspect(&self) -> f32 {
        self.aspect
    }

    pub fn build_matrix(&self) -> Matrix4<f32> {
        perspective(self.fov_y, self.aspect, self.znear, self.zfar)
    }

    pub fn base_fov(&self) -> Rad<f32> {
        self.base_fov
    }

    pub fn set_target_fov(&mut self, fov: Rad<f32>) {
        self.target_fov = fov;
    }

    pub fn animate(&mut self, dt: f32) {
        let rate = 10.0;
        let lerp = 1.0 - (-rate * dt).exp();
        self.fov_y = Rad(self.fov_y.0 + (self.target_fov.0 - self.fov_y.0) * lerp);
    }

    pub fn ray_direction(&self, camera: &Camera, screen: (f32, f32)) -> Vector3<f32> {
        let ndc_x = screen.0 * 2.0 - 1.0;
        let ndc_y = 1.0 - screen.1 * 2.0;

        let proj = self.build_matrix();
        let view = Matrix4::look_to_rh(camera.position, camera.direction(), Camera::UP);
        if let (Some(inv_proj), Some(inv_view)) = (proj.invert(), view.invert()) {
            let clip = Vector4::new(ndc_x, ndc_y, -1.0, 1.0);
            let mut view_dir = inv_proj * clip;
            if view_dir.w.abs() > 1e-6 {
                view_dir /= view_dir.w;
            }
            let mut dir = inv_view * Vector4::new(view_dir.x, view_dir.y, -1.0, 0.0);
            if dir.w.abs() > 1e-6 {
                dir /= dir.w;
            }
            Vector3::new(dir.x, dir.y, dir.z).normalize()
        } else {
            camera.direction()
        }
    }
}

pub struct CameraController {
    base_speed: f32,
    sprint_multiplier: f32,
    sensitivity: f32,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
    is_jump_pressed: bool,
    is_sprint_pressed: bool,
    scroll: f32,
    velocity_y: f32,
    is_on_ground: bool,
    horizontal_velocity: Vector3<f32>,
    pub noclip: bool,
}

impl CameraController {
    pub fn sensitivity(&self) -> f32 {
        self.sensitivity
    }

    pub fn set_sensitivity(&mut self, value: f32) {
        self.sensitivity = value.clamp(0.0005, 0.02);
    }

    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            base_speed: speed,
            sprint_multiplier: 1.6,
            sensitivity,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
            is_jump_pressed: false,
            is_sprint_pressed: false,
            scroll: 0.0,
            velocity_y: 0.0,
            is_on_ground: true, // Start on ground
            horizontal_velocity: Vector3::new(0.0, 0.0, 0.0),
            noclip: false,
        }
    }

    pub fn toggle_noclip(&mut self) {
        self.noclip = !self.noclip;
    }

    pub fn process_events(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    let is_pressed = event.state == ElementState::Pressed;
                    match keycode {
                        KeyCode::KeyW => self.is_forward_pressed = is_pressed,
                        KeyCode::KeyS => self.is_backward_pressed = is_pressed,
                        KeyCode::KeyA => self.is_left_pressed = is_pressed,
                        KeyCode::KeyD => self.is_right_pressed = is_pressed,
                        KeyCode::Space => self.is_jump_pressed = is_pressed,
                        KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                            self.is_sprint_pressed = is_pressed
                        }
                        _ => return false,
                    }
                    return true;
                }
                false
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y * 0.1,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.01,
                };
                self.scroll += scroll_amount;
                true
            }
            _ => false,
        }
    }

    pub fn process_mouse(&mut self, delta: (f64, f64), camera: &mut Camera) {
        let (dx, dy) = delta;
        camera.yaw += Rad(dx as f32 * self.sensitivity);
        camera.pitch += Rad(-dy as f32 * self.sensitivity);

        let half_pi = std::f32::consts::FRAC_PI_2 - 0.01;
        camera.pitch.0 = camera.pitch.0.clamp(-half_pi, half_pi);
    }

    pub fn is_sprinting(&self) -> bool {
        !self.noclip && self.is_sprint_pressed && self.horizontal_velocity.magnitude2() > 0.05
    }

    pub fn update_camera(
        &mut self,
        camera: &mut Camera,
        dt: f32,
        check_collision: impl Fn(cgmath::Point3<f32>) -> bool,
    ) {
        if self.noclip {
            // Noclip mode - free flight
            let speed_multiplier = if self.is_sprint_pressed {
                self.sprint_multiplier
            } else {
                1.0
            };
            let mut direction = Vector3::new(0.0, 0.0, 0.0);
            if self.is_forward_pressed {
                direction += camera.direction();
            }
            if self.is_backward_pressed {
                direction -= camera.direction();
            }
            if self.is_left_pressed {
                direction -= camera.right();
            }
            if self.is_right_pressed {
                direction += camera.right();
            }
            if self.is_jump_pressed {
                direction += Camera::UP;
            }

            if direction.magnitude2() > 0.0 {
                direction = direction.normalize();
            }

            self.horizontal_velocity = Vector3::new(0.0, 0.0, 0.0);
            camera.position += direction * self.base_speed * speed_multiplier * dt;
        } else {
            // Normal mode - with gravity and collision
            // Handle horizontal movement
            let forward = {
                let mut f = camera.direction();
                f.y = 0.0;
                if f.magnitude2() > 0.0 {
                    f.normalize()
                } else {
                    Vector3::new(0.0, 0.0, 1.0)
                }
            };
            let right = forward.cross(Camera::UP).normalize();

            let mut horizontal = Vector3::new(0.0, 0.0, 0.0);
            if self.is_forward_pressed {
                horizontal += forward;
            }
            if self.is_backward_pressed {
                horizontal -= forward;
            }
            if self.is_left_pressed {
                horizontal -= right;
            }
            if self.is_right_pressed {
                horizontal += right;
            }

            if horizontal.magnitude2() > 0.0 {
                horizontal = horizontal.normalize();
            }

            let speed_multiplier = if self.is_sprint_pressed {
                self.sprint_multiplier
            } else {
                1.0
            };
            let target_velocity = horizontal * self.base_speed * speed_multiplier;
            let accel = 12.0;
            let lerp_factor = 1.0 - (-accel * dt).exp();
            self.horizontal_velocity = self.horizontal_velocity
                + (target_velocity - self.horizontal_velocity) * lerp_factor;

            let mut horizontal_movement = self.horizontal_velocity * dt;
            if horizontal_movement.magnitude2() < 1e-6 {
                horizontal_movement = Vector3::new(0.0, 0.0, 0.0);
            }

            // Apply horizontal movement with collision
            let new_pos_x = camera.position + Vector3::new(horizontal_movement.x, 0.0, 0.0);
            if !check_collision(new_pos_x) {
                camera.position = new_pos_x;
            } else {
                self.horizontal_velocity.x = 0.0;
            }

            let new_pos_z = camera.position + Vector3::new(0.0, 0.0, horizontal_movement.z);
            if !check_collision(new_pos_z) {
                camera.position = new_pos_z;
            } else {
                self.horizontal_velocity.z = 0.0;
            }

            // Check if on ground (check slightly below feet)
            let ground_check = camera.position + Vector3::new(0.0, -0.05, 0.0);
            self.is_on_ground = check_collision(ground_check);

            // Jumping
            if self.is_jump_pressed && self.is_on_ground {
                self.velocity_y = JUMP_VELOCITY;
                self.is_on_ground = false;
            }

            // Apply gravity
            if !self.is_on_ground {
                self.velocity_y += GRAVITY * dt;
            } else {
                self.velocity_y = 0.0;
            }

            // Apply vertical movement
            let vertical_movement = self.velocity_y * dt;
            let new_pos_y = camera.position + Vector3::new(0.0, vertical_movement, 0.0);
            if !check_collision(new_pos_y) {
                camera.position = new_pos_y;
            } else {
                if self.velocity_y < 0.0 {
                    self.is_on_ground = true;
                    // If player is stuck inside a block, try to push them out
                    // Limit iterations to prevent performance issues
                    if check_collision(camera.position) {
                        let mut resolve_pos = camera.position;
                        const MAX_RESOLVE_ITERATIONS: i32 = 15;
                        const RESOLVE_STEP: f32 = 0.05; // Increased step size for faster resolution

                        for _ in 0..MAX_RESOLVE_ITERATIONS {
                            if !check_collision(resolve_pos) {
                                break;
                            }
                            resolve_pos.y += RESOLVE_STEP;
                        }
                        camera.position = resolve_pos;
                    }
                }
                self.velocity_y = 0.0;
            }
        }

        camera.position += Camera::UP * self.scroll;
        self.scroll = 0.0;
    }

    pub fn reset_motion(&mut self) {
        self.horizontal_velocity = Vector3::new(0.0, 0.0, 0.0);
        self.velocity_y = 0.0;
        self.scroll = 0.0;
    }
}
