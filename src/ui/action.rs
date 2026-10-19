/// What a screen asks the application shell to do.
///
/// Screens handle their own keys and mutate their own state. When a key crosses
/// a screen boundary, the screen returns one of these actions instead of acting
/// on the whole application.
#[derive(Debug, Clone)]
pub enum Action {
    /// Close the application.
    Quit,
    /// Scan the chosen repository path.
    ScanRepository(String),
    /// Return from the results to the repository picker.
    BackToPicker,
}
