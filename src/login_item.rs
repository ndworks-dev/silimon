//! Opening the app at login through the system's login items.

use objc2_service_management::{SMAppService, SMAppServiceStatus};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginState {
    Off,
    On,
    /// Registered, but the user still has to allow it in System Settings.
    NeedsApproval,
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Register,
    Unregister,
    None,
}

fn action_for(state: LoginState) -> Action {
    match state {
        LoginState::Off => Action::Register,
        LoginState::On => Action::Unregister,
        LoginState::NeedsApproval => Action::None,
    }
}

fn state_of(status: SMAppServiceStatus) -> LoginState {
    if status == SMAppServiceStatus::Enabled {
        LoginState::On
    } else if status == SMAppServiceStatus::RequiresApproval {
        LoginState::NeedsApproval
    } else {
        LoginState::Off
    }
}

pub fn state() -> LoginState {
    state_of(unsafe { SMAppService::mainAppService().status() })
}

/// Switches the login item and returns the resulting state.
pub fn toggle() -> Result<LoginState, String> {
    let service = unsafe { SMAppService::mainAppService() };
    let result = match action_for(state_of(unsafe { service.status() })) {
        Action::Register => unsafe { service.registerAndReturnError() },
        Action::Unregister => unsafe { service.unregisterAndReturnError() },
        Action::None => Ok(()),
    };
    result.map_err(|error| error.localizedDescription().to_string())?;
    Ok(state())
}

pub fn open_settings() {
    unsafe { SMAppService::openSystemSettingsLoginItems() };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_follows_the_current_state() {
        assert_eq!(action_for(LoginState::Off), Action::Register);
        assert_eq!(action_for(LoginState::On), Action::Unregister);
    }

    #[test]
    fn pending_approval_is_kept_registered() {
        assert_eq!(action_for(LoginState::NeedsApproval), Action::None);
    }

    #[test]
    fn system_statuses_map_to_states() {
        assert_eq!(state_of(SMAppServiceStatus::Enabled), LoginState::On);
        assert_eq!(state_of(SMAppServiceStatus::RequiresApproval), LoginState::NeedsApproval);
        assert_eq!(state_of(SMAppServiceStatus::NotRegistered), LoginState::Off);
        assert_eq!(state_of(SMAppServiceStatus::NotFound), LoginState::Off);
    }
}
