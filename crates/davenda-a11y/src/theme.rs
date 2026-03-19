use crate::A11yError;
use crate::validation::validate_ratio;

#[derive(Debug, Clone, PartialEq)]
pub struct ThemeAccessibilityContract {
    pub body_text_contrast: f32,
    pub muted_text_contrast: f32,
    pub focus_indicator_contrast: f32,
    pub honors_reduced_motion: bool,
    pub visible_focus_states: bool,
}

impl ThemeAccessibilityContract {
    pub fn new(
        body_text_contrast: f32,
        muted_text_contrast: f32,
        focus_indicator_contrast: f32,
        honors_reduced_motion: bool,
        visible_focus_states: bool,
    ) -> Result<Self, A11yError> {
        validate_ratio("body_text_contrast", body_text_contrast)?;
        validate_ratio("muted_text_contrast", muted_text_contrast)?;
        validate_ratio("focus_indicator_contrast", focus_indicator_contrast)?;

        Ok(Self {
            body_text_contrast,
            muted_text_contrast,
            focus_indicator_contrast,
            honors_reduced_motion,
            visible_focus_states,
        })
    }

    pub fn meets_platform_baseline(&self) -> bool {
        self.body_text_contrast >= 4.5
            && self.muted_text_contrast >= 3.0
            && self.focus_indicator_contrast >= 3.0
            && self.honors_reduced_motion
            && self.visible_focus_states
    }
}
