use crate::geometry::FeatureRecord;

#[derive(Debug, Clone, Copy)]
pub struct Epsilon {
    pub zero_relative: f64,
    pub zero_absolute: f64,
    pub zero_angle: f64,
}

impl Epsilon {
    pub fn from_features(features: &[FeatureRecord]) -> Self {
        let mut min_x = 0.0;
        let mut min_y = 0.0;
        let mut max_x = 1.0;
        let mut max_y = 1.0;

        if !features.is_empty() {
            let mut first = true;
            for feature in features {
                for c in feature.geometry.all_coords() {
                    if first {
                        min_x = c.x;
                        max_x = c.x;
                        min_y = c.y;
                        max_y = c.y;
                        first = false;
                    } else {
                        min_x = min_x.min(c.x);
                        min_y = min_y.min(c.y);
                        max_x = max_x.max(c.x);
                        max_y = max_y.max(c.y);
                    }
                }
            }
        }

        let delta_x = min_x.abs() + max_x.abs();
        let delta_y = min_y.abs() + max_y.abs();
        let mut dynamic_xy = delta_x.max(delta_y);
        if dynamic_xy == 0.0 {
            dynamic_xy = 1.0e-15;
        }

        let log_loss = (dynamic_xy.log10() + 1.0).floor() as i32;
        let max_digit = 15;
        let security = 2;
        let abs_digit = max_digit - security;
        let rel_digit = max_digit - log_loss - security;

        let zero_relative = 10f64.powi(-rel_digit);
        let zero_absolute = 10f64.powi(-abs_digit);
        let zero_angle = (0.0001f64).to_radians();

        Self {
            zero_relative,
            zero_absolute,
            zero_angle,
        }
    }
}
