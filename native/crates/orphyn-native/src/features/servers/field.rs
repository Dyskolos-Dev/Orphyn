//! `TextField` (`design/controls/TextField.tsx`), same DOM.
use dioxus::prelude::*;

#[component]
pub fn TextField(
    label: String,
    value: Signal<String>,
    #[props(default = "text")] kind: &'static str,
    #[props(default)] placeholder: String,
    #[props(default)] hint: String,
    error: Option<String>,
    #[props(default = "large")] size: &'static str,
    trailing: Option<Element>,
    on_input: Option<EventHandler<String>>,
    on_enter: Option<EventHandler<()>>,
) -> Element {
    let mut value = value;
    let invalid_flag = error.is_some();
    let invalid = if error.is_some() { " jr-field--invalid" } else { "" };
    rsx! {
        div { class: "jr-field jr-field--{size}{invalid}",
            label { class: "jr-field__label", "{label}" }
            div { class: "jr-field__box",
                input {
                    class: "jr-field__input",
                    r#type: kind,
                    placeholder,
                    value: "{value}",
                    "aria-invalid": invalid_flag.to_string(),
                    oninput: move |e| {
                        value.set(e.value());
                        if let Some(h) = &on_input { h.call(e.value()); }
                    },
                    onkeydown: move |e| {
                        if e.key() == Key::Named(NamedKey::Enter) {
                            if let Some(h) = &on_enter { h.call(()); }
                        }
                    },
                }
                if let Some(tr) = trailing {
                    span { class: "jr-field__trailing", {tr} }
                }
            }
            if let Some(e) = error.clone() {
                span { class: "jr-field__error", role: "alert", "{e}" }
            } else if !hint.is_empty() {
                span { class: "jr-field__hint", "{hint}" }
            }
        }
    }
}
