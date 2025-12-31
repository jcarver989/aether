use dioxus::prelude::*;
use crate::components::layout::{Stack, Space};

const HEADER_SVG: Asset = asset!("/assets/header.svg");

#[component]
pub fn Hero() -> Element {
    rsx! {
        Stack {
            gap: Space::S8,
            p: Space::S8,
            class: "items-center justify-center min-h-screen",
            img {
                src: HEADER_SVG,
                class: "max-w-5xl w-full"
            }
            Stack {
                gap: Space::S4,
                class: "w-100 text-left",
                a {
                    href: "https://dioxuslabs.com/learn/0.7/",
                    class: "text-2xl text-white no-underline p-3 border border-white rounded-lg hover:bg-white/10 transition-colors",
                    "📚 Learn Dioxus"
                }
                a {
                    href: "https://dioxuslabs.com/awesome",
                    class: "text-2xl text-white no-underline p-3 border border-white rounded-lg hover:bg-white/10 transition-colors",
                    "🚀 Awesome Dioxus"
                }
                a {
                    href: "https://github.com/dioxus-community/",
                    class: "text-2xl text-white no-underline p-3 border border-white rounded-lg hover:bg-white/10 transition-colors",
                    "📡 Community Libraries"
                }
                a {
                    href: "https://github.com/DioxusLabs/sdk",
                    class: "text-2xl text-white no-underline p-3 border border-white rounded-lg hover:bg-white/10 transition-colors",
                    "⚙️ Dioxus Development Kit"
                }
                a {
                    href: "https://marketplace.visualstudio.com/items?itemName=DioxusLabs.dioxus",
                    class: "text-2xl text-white no-underline p-3 border border-white rounded-lg hover:bg-white/10 transition-colors",
                    "💫 VSCode Extension"
                }
                a {
                    href: "https://discord.gg/XgGxMSkvUM",
                    class: "text-2xl text-white no-underline p-3 border border-white rounded-lg hover:bg-white/10 transition-colors",
                    "👋 Community Discord"
                }
            }
        }
    }
}
