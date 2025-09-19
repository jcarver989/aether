use iocraft::prelude::*;

#[derive(Default, Props)]
pub struct InputProps {
    label: String,
    pub value: Option<State<String>>,
    has_focus: bool,
    multiline: bool,
}

#[component]
pub fn Input(props: &InputProps) -> impl Into<AnyElement<'static>> {
    let Some(mut value) = props.value else {
        panic!("value is required");
    };

    element! {
        View(
            border_style: if props.has_focus { BorderStyle::Round } else { BorderStyle::None },
            border_color: Color::Blue,
            padding: if props.has_focus { 0 } else { 1 },
        ) {
            View(width: 15) {
                Text(content: format!("{}: ", props.label))
            }
            View(
                background_color: Color::DarkGrey,
                width: 30,
                height: if props.multiline { 5 } else { 1 },
            ) {
                TextInput(
                    has_focus: props.has_focus,
                    value: value.to_string(),
                    on_change: move |new_value| value.set(new_value),
                    multiline: props.multiline,
                )
            }
        }
    }
}
