use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Space {
    None,
    S1,
    S2,
    S3,
    S4,
}

impl Space {
    pub fn to_tailwind(self) -> &'static str {
        match self {
            Space::None => "0",
            Space::S1 => "1",
            Space::S2 => "2",
            Space::S3 => "3",
            Space::S4 => "4",
        }
    }

    pub fn gap_class(self) -> String {
        format!("gap-{}", self.to_tailwind())
    }

    pub fn p_class(self) -> String {
        format!("p-{}", self.to_tailwind())
    }
}

#[component]
pub fn Stack(
    #[props(default = Space::None)] gap: Space,
    #[props(default = Space::None)] p: Space,
    #[props(default = "".to_string())] class: String,
    #[props(default)] id: Option<String>,
    children: Element,
) -> Element {
    let gap_class = gap.gap_class();
    let p_class = p.p_class();

    rsx! {
        div {
            class: "flex flex-col {gap_class} {p_class} {class}",
            id: id,
            {children}
        }
    }
}

#[component]
pub fn Inline(
    #[props(default = Space::None)] gap: Space,
    #[props(default = Space::None)] p: Space,
    #[props(default = "items-center".to_string())] align: String,
    #[props(default = false)] wrap: bool,
    #[props(default = "".to_string())] class: String,
    children: Element,
) -> Element {
    let gap_class = gap.gap_class();
    let p_class = p.p_class();
    let wrap_class = if wrap { "flex-wrap" } else { "flex-nowrap" };

    rsx! {
        div {
            class: "flex flex-row {align} {wrap_class} {gap_class} {p_class} {class}",
            {children}
        }
    }
}

#[component]
pub fn Card(
    #[props(default = Space::S4)] p: Space,
    #[props(default = "rounded-xl".to_string())] radius: String,
    #[props(default = "bg-bg-secondary border border-border-default".to_string())]
    background: String,
    #[props(default = "".to_string())] class: String,
    children: Element,
) -> Element {
    let p_class = p.p_class();

    rsx! {
        div {
            class: "{background} {radius} {p_class} {class}",
            {children}
        }
    }
}
