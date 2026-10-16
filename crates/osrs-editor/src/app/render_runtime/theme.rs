//! Rendering owns pipeline candidates, including asynchronous WebGPU validation.
use super::RenderRuntime;
use osrs_hd::theme::shader::ShaderExtension;
use std::task::Poll;
impl RenderRuntime {
    pub(in crate::app) fn theme_shaders_match(&self, shaders: &[ShaderExtension]) -> bool {
        self.theme_shaders == shaders
    }
    pub(in crate::app) fn theme_needs_restore(&self, shaders: &[ShaderExtension]) -> bool {
        self.theme_shaders != shaders && self.rejected_theme_shaders.as_deref() != Some(shaders)
    }
    pub(in crate::app) fn prepare_theme(
        &mut self,
        shaders: &[ShaderExtension],
    ) -> Result<(), String> {
        self.rejected_theme_shaders = None;
        let candidate = match self.scene.prepare_theme_shaders(shaders) {
            Ok(candidate) => candidate,
            Err(error) => {
                self.rejected_theme_shaders = Some(shaders.to_vec());
                return Err(error);
            }
        };
        self.prepared_theme_shaders = shaders.to_vec();
        self.theme_preparation = Some(candidate);
        Ok(())
    }
    pub(in crate::app) fn poll_theme_preparation(&mut self) -> Poll<Result<(), String>> {
        let Some(candidate) = &mut self.theme_preparation else {
            return Poll::Ready(Err("No appearance preparation is active".into()));
        };
        let result = candidate.poll();
        if matches!(&result, Poll::Ready(Err(_))) {
            self.rejected_theme_shaders = Some(self.prepared_theme_shaders.clone());
            self.theme_preparation = None;
        }
        result
    }
    pub(in crate::app) fn cancel_theme_preparation(&mut self) {
        self.theme_preparation = None;
    }
    pub(in crate::app) fn install_prepared_theme(&mut self) -> Result<(), String> {
        let candidate = self
            .theme_preparation
            .take()
            .ok_or("No prepared appearance")?;
        self.scene.install_theme_shaders(candidate)?;
        self.theme_shaders.clone_from(&self.prepared_theme_shaders);
        self.rejected_theme_shaders = None;
        self.reuse.invalidate();
        Ok(())
    }
}
