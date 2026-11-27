uniffi::setup_scaffolding!();

#[uniffi::export]
pub fn core_version() -> String {
    notes_app::VERSION.to_owned()
}
