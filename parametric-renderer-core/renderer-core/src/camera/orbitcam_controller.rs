use glam::Quat;
use glamour::{Angle, Point3, Vector2, Vector3};
use winit::event::MouseButton;
use winit_input_helper::WinitInputHelper;

use super::{
    camera_controller::{
        CursorCapture, GeneralController, GeneralControllerSettings, IsCameraController,
    },
    Camera,
};

pub struct OrbitcamController {
    pub center: Point3,
    pub pitch: Angle,
    pub yaw: Angle,
    pub distance: f32,
}

impl OrbitcamController {
    pub fn new(controller: GeneralController) -> Self {
        let center = controller.position
            + controller.orientation * (Camera::forward() * controller.distance_to_center);

        let (pitch, yaw, _) = controller.orientation.to_euler(glam::EulerRot::XYZ);

        Self {
            center,
            pitch: Angle::from(pitch),
            yaw: Angle::from(yaw),
            distance: controller.distance_to_center,
        }
    }
    pub fn update(
        &mut self,
        input: &WinitInputHelper,
        delta_time: f32,
        settings: &GeneralControllerSettings,
    ) -> CursorCapture {
        let mut cursor_capture = CursorCapture::Free;
        if input.mouse_held(MouseButton::Right) {
            self.update_orientation(Vector2::from(input.mouse_diff()), settings);
            cursor_capture = CursorCapture::LockedAndHidden;
        }

        if input.mouse_held(MouseButton::Middle) {
            self.update_pan_position(Vector2::from(input.mouse_diff()), delta_time, settings);
            cursor_capture = CursorCapture::LockedAndHidden;
        }
        cursor_capture
    }

    fn update_orientation(&mut self, mouse_delta: Vector2, settings: &GeneralControllerSettings) {
        self.set_pitch_yaw(
            self.pitch - Angle::new(mouse_delta.y * settings.rotation_sensitivity),
            self.yaw - Angle::new(mouse_delta.x * settings.rotation_sensitivity),
        );
    }

    fn set_pitch_yaw(&mut self, new_pitch: Angle, new_yaw: Angle) {
        const TWO_PI: f32 = std::f32::consts::PI * 2.0;
        let max_pitch = 88f32;
        self.pitch = new_pitch
            .min(Angle::from_degrees(max_pitch))
            .max(Angle::from_degrees(-max_pitch));
        self.yaw = Angle::new(new_yaw.radians.rem_euclid(TWO_PI));
    }

    fn update_pan_position(
        &mut self,
        direction: Vector2,
        delta_time: f32,
        settings: &GeneralControllerSettings,
    ) {
        let horizontal_movement = self.orientation() * (Camera::right() * direction.x * 1.0);
        let vertical_movement = self.orientation() * (Camera::up() * direction.y * -1.0);
        self.center += horizontal_movement * settings.pan_speed * delta_time;
        self.center += vertical_movement * settings.pan_speed * delta_time;
    }
}

impl IsCameraController for OrbitcamController {
    fn position(&self) -> Point3 {
        self.center + self.orientation() * Vector3::new(0.0, 0.0, self.distance)
    }

    fn orientation(&self) -> Quat {
        Quat::from_euler(
            glam::EulerRot::XYZ,
            self.pitch.radians,
            self.yaw.radians,
            0.0,
        )
    }
}
