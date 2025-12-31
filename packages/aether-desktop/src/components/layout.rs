use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Space {
    None,
    S1,
    S2,
    S3,
    S4,
    S5,
    S6,
    S8,
    S10,
    S12,
    S16,
    S20,
    S24,
}

impl Space {
    pub fn to_tailwind(&self) -> &'static str {
        match self {
            Space::None => "0",
            Space::S1 => "1",
            Space::S2 => "2",
            Space::S3 => "3",
            Space::S4 => "4",
            Space::S5 => "5",
            Space::S6 => "6",
            Space::S8 => "8",
            Space::S10 => "10",
            Space::S12 => "12",
            Space::S16 => "16",
            Space::S20 => "20",
            Space::S24 => "24",
        }
    }

    pub fn gap_class(&self) -> String {
        format!("gap-{}", self.to_tailwind())
    }

    pub fn p_class(&self) -> String {
        format!("p-{}", self.to_tailwind())
    }

    pub fn px_class(&self) -> String {
        format!("px-{}", self.to_tailwind())
    }

    pub fn py_class(&self) -> String {
        format!("py-{}", self.to_tailwind())
    }
}

#[component]
pub fn Stack(
    #[props(default = Space::None)] gap: Space,
    #[props(default = Space::None)] p: Space,
    #[props(default = "")] class: String,
    children: Element,
) -> Element {
    let gap_class = gap.gap_class();
    let p_class = p.p_class();

    rsx! {
        div {
            class: "flex flex-col {gap_class} {p_class} {class}",
            {children}
        }
    }
}

#[component]
pub fn Inline(
    #[props(default = Space::None)] gap: Space,
    #[props(default = Space::None)] p: Space,
    #[props(default = "items-center")] align: String,
    #[props(default = false)] wrap: bool,
    #[props(default = "")] class: String,
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
pub fn Container(
    #[props(default = Space::S4)] p: Space,
    #[props(default = "")] class: String,
    children: Element,
) -> Element {
    let p_class = p.p_class();

    rsx! {
        div {
            class: "w-full max-w-7xl mx-auto {p_class} {class}",
            {children}
        }
    }
}

#[component]
pub fn Card(
    #[props(default = Space::S4)] p: Space,
    #[props(default = "rounded-xl")] radius: String,
    #[props(default = "bg-bg-secondary border border-border-default")] background: String,
    #[props(default = "")] class: String,
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

#[component]
pub fn SectionHeader(
    title: String,
    #[props(default = None)] subtitle: Option<String>,
    #[props(default = None)] actions: Option<Element>,
) -> Element {
    rsx! {
        Inline {
            gap: Space::S4,
            class: "justify-between mb-4",
            Stack {
                gap: Space::S1,
                h2 { class: "text-xl font-semibold", "{title}" }
                if let Some(subtitle) = subtitle {
                    p { class: "text-sm text-text-secondary", "{subtitle}" }
                }
            }
            if let Some(actions) = actions {
                Inline { gap: Space::S2, {actions} }
            }
        }
    }
}

