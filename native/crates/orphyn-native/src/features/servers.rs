//! Servers: the animated onboarding (`OnboardingPage.tsx`), the three-step connect flow
//! (`ConnectFlow.tsx`), the add-server sheet and the sidebar server switcher.
mod avatar;
mod connect;
mod field;
mod onboarding;
mod sheet;
mod switcher;

pub use avatar::UserAvatar;
pub use connect::ConnectFlow;
pub use field::TextField;
pub use onboarding::OnboardingPage;
pub use sheet::{AddServerSheet, Sheet};
pub use switcher::ServerSwitcher;
