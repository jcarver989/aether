---
name: frontend-design
description: Design system tokens and components for the MCP Gateway frontend. Use when working on frontend UI, styling, or adding new components.
allowed-tools:
  - Read
  - Glob
  - Grep
---

# Design Tokens

This document outlines the design system tokens and component libraries used in the MCP Gateway frontend.

## Component Libraries

We use two primary component libraries:

1. **ShadCN UI** - Core UI components, blocks, and charts ([ui.shadcn.com](https://ui.shadcn.com))
2. **Magic UI** - Animated and special effect components ([magicui.design](https://magicui.design))

Always prefer these libraries over custom implementations.

---

## Checking Installed Components

**Before adding a new component, always check if it's already installed:**

```bash
# Check for a specific component (e.g., button, dialog, card)
ls frontend/src/components/ui/<component-name>.tsx

# List all installed UI components
ls frontend/src/components/ui/
```

**Using Glob tool:**
```
pattern: "**/components/ui/<component-name>.tsx"
```

**Example workflow:**
1. Check if component exists: `ls frontend/src/components/ui/dialog.tsx`
2. If not found, install: `pnpm dlx shadcn@latest add dialog`
3. If found, import and use directly

**Currently installed components** can be found at: `frontend/src/components/ui/`

---

## ShadCN UI Components

Install components using: `pnpm dlx shadcn@latest add <component-name>`

### Core Components

| Component | Description | Install |
|-----------|-------------|---------|
| `accordion` | Vertically stacked interactive headings | `shadcn@latest add accordion` |
| `alert` | Callout for user attention | `shadcn@latest add alert` |
| `alert-dialog` | Modal dialog for important content | `shadcn@latest add alert-dialog` |
| `avatar` | Image with fallback for user representation | `shadcn@latest add avatar` |
| `badge` | Small status indicator | `shadcn@latest add badge` |
| `breadcrumb` | Navigation hierarchy | `shadcn@latest add breadcrumb` |
| `button` | Interactive button element | `shadcn@latest add button` |
| `calendar` | Date field component | `shadcn@latest add calendar` |
| `card` | Container with header, content, footer | `shadcn@latest add card` |
| `carousel` | Swipeable content carousel | `shadcn@latest add carousel` |
| `checkbox` | Toggle control | `shadcn@latest add checkbox` |
| `collapsible` | Expandable/collapsible panel | `shadcn@latest add collapsible` |
| `combobox` | Autocomplete input with suggestions | `shadcn@latest add combobox` |
| `command` | Command palette menu | `shadcn@latest add command` |
| `context-menu` | Right-click menu | `shadcn@latest add context-menu` |
| `dialog` | Modal overlay window | `shadcn@latest add dialog` |
| `drawer` | Slide-out panel | `shadcn@latest add drawer` |
| `dropdown-menu` | Menu triggered by button | `shadcn@latest add dropdown-menu` |
| `form` | Form with React Hook Form + Zod | `shadcn@latest add form` |
| `hover-card` | Preview content on hover | `shadcn@latest add hover-card` |
| `input` | Form input field | `shadcn@latest add input` |
| `input-otp` | One-time password input | `shadcn@latest add input-otp` |
| `label` | Accessible label for controls | `shadcn@latest add label` |
| `menubar` | Persistent menu bar | `shadcn@latest add menubar` |
| `navigation-menu` | Navigation link collection | `shadcn@latest add navigation-menu` |
| `pagination` | Page navigation controls | `shadcn@latest add pagination` |
| `popover` | Rich content portal | `shadcn@latest add popover` |
| `progress` | Task completion indicator | `shadcn@latest add progress` |
| `radio-group` | Single-select button group | `shadcn@latest add radio-group` |
| `resizable` | Resizable panel layouts | `shadcn@latest add resizable` |
| `scroll-area` | Custom scrollbar styling | `shadcn@latest add scroll-area` |
| `select` | Dropdown selection | `shadcn@latest add select` |
| `separator` | Visual divider | `shadcn@latest add separator` |
| `sheet` | Complementary content panel | `shadcn@latest add sheet` |
| `sidebar` | Composable sidebar component | `shadcn@latest add sidebar` |
| `skeleton` | Loading placeholder | `shadcn@latest add skeleton` |
| `slider` | Range input | `shadcn@latest add slider` |
| `sonner` | Toast notifications | `shadcn@latest add sonner` |
| `spinner` | Loading indicator | `shadcn@latest add spinner` |
| `switch` | Toggle switch | `shadcn@latest add switch` |
| `table` | Responsive table | `shadcn@latest add table` |
| `tabs` | Tabbed content sections | `shadcn@latest add tabs` |
| `textarea` | Multi-line text input | `shadcn@latest add textarea` |
| `toast` | Temporary message | `shadcn@latest add toast` |
| `toggle` | Two-state button | `shadcn@latest add toggle` |
| `toggle-group` | Group of toggle buttons | `shadcn@latest add toggle-group` |
| `tooltip` | Information popup on hover | `shadcn@latest add tooltip` |

### ShadCN Blocks

Pre-built, copy-paste templates. Install using: `pnpm dlx shadcn@latest add <block-name>`

**Dashboard:**
- `dashboard-01` - Dashboard with sidebar, charts, and data table

**Sidebar Variants:**
- `sidebar-01` to `sidebar-16` - Various sidebar configurations (collapsible, submenus, icons, file trees, calendars)

**Authentication:**
- `login-01` to `login-05` - Login page variants
- `signup-01` to `signup-05` - Signup page variants
- `otp-01` to `otp-05` - OTP verification variants

**Calendar/Date Picker:**
- `calendar-01` to `calendar-32` - Calendar and date picker variants (range, time, localized, presets)

### ShadCN Charts

Built on Recharts. Install: `pnpm dlx shadcn@latest add chart`

**Chart Types:**

| Type | Variants | Example Block |
|------|----------|---------------|
| Area | default, gradient, stacked, step, linear, icons, legend, axes | `chart-area-default` |
| Bar | default, horizontal, stacked, multiple, negative, mixed, active | `chart-bar-default` |
| Line | default, dots, multiple, step, linear, labels | `chart-line-default` |
| Pie | simple, donut, legend, labels, stacked | `chart-pie-simple` |
| Radar | default, dots, grid variants, icons, legend | `chart-radar-default` |
| Radial | simple, grid, label, text, shape, stacked | `chart-radial-simple` |

**Chart Usage:**
```tsx
import { ChartContainer, ChartTooltip, ChartTooltipContent } from '@/components/ui/chart';
import { Bar, BarChart, CartesianGrid, XAxis } from 'recharts';

const chartConfig = {
  desktop: { label: "Desktop", color: "var(--chart-1)" },
  mobile: { label: "Mobile", color: "var(--chart-2)" },
};

<ChartContainer config={chartConfig} className="min-h-[200px]">
  <BarChart data={data} accessibilityLayer>
    <CartesianGrid vertical={false} />
    <XAxis dataKey="month" />
    <ChartTooltip content={<ChartTooltipContent />} />
    <Bar dataKey="desktop" fill="var(--color-desktop)" />
  </BarChart>
</ChartContainer>
```

---

## Magic UI Components

Install components from Magic UI registry. Docs: [magicui.design/docs/components](https://magicui.design/docs/components)

### Installed Components

| Component | Import | Usage |
|-----------|--------|-------|
| `ShimmerButton` | `@/components/ui/shimmer-button` | Animated CTA buttons |
| `BlurFade` | `@/components/ui/blur-fade` | Fade-in animations |
| `BorderBeam` | `@/components/ui/border-beam` | Animated border highlight |
| `AnimatedGradientText` | `@/components/ui/animated-gradient-text` | Gradient text animation |
| `Particles` | `@/components/ui/particles` | Background particle effects |

### Additional Magic UI Components

**Core Components:**
- `marquee` - Infinite scrolling content
- `bento-grid` - Feature showcase layout
- `globe` - 3D interactive globe
- `dock` - macOS-style dock
- `terminal` - Terminal emulator display
- `hero-video-dialog` - Video modal for hero sections
- `animated-list` - Animated list items
- `orbiting-circles` - Circular orbit animation
- `avatar-circles` - Overlapping avatar group
- `icon-cloud` - Floating icon cloud
- `lens` - Magnifying lens effect
- `pointer` - Custom cursor effects
- `dotted-map` - Animated dotted world map

**Special Effects:**
- `animated-beam` - Beam animation between elements
- `shine-border` - Shining border effect
- `magic-card` - Interactive card with lighting
- `meteors` - Meteor shower animation
- `confetti` - Celebration confetti

**Text Animations:**
- `text-animate` - Various text animations
- `typing-animation` - Typewriter effect
- `aurora-text` - Aurora borealis text
- `number-ticker` - Animated number counter
- `animated-shiny-text` - Shiny text effect
- `text-reveal` - Text reveal on scroll
- `word-rotate` - Rotating word animation
- `sparkles-text` - Sparkle effect on text
- `morphing-text` - Shape-morphing text

**Buttons:**
- `rainbow-button` - Rainbow gradient button
- `ripple-button` - Material-style ripple

**Backgrounds:**
- `flickering-grid` - Animated grid pattern
- `retro-grid` - Retro perspective grid
- `interactive-grid-pattern` - Mouse-interactive grid

### Magic UI Example Usage

```tsx
import { ShimmerButton } from '@/components/ui/shimmer-button';
import { BlurFade } from '@/components/ui/blur-fade';

// Shimmer button with brand colors
<ShimmerButton
  shimmerColor="oklch(72% 0.151 162.68)"
  background="oklch(60% 0.139 162.68)"
>
  Get Started
</ShimmerButton>

// Fade-in animation
<BlurFade delay={0.25}>
  <h1>Welcome</h1>
</BlurFade>
```

---

## Color System

We use OKLCH color space for perceptual uniformity. All colors are defined as CSS custom properties in `index.css`.

### Brand Colors

| Token | Light Mode | Dark Mode | Usage |
|-------|-----------|-----------|-------|
| `--primary` | `oklch(60% 0.139 162.68)` | `oklch(72% 0.151 162.68)` | Primary actions, links |
| `--primary-foreground` | `oklch(98% 0.020 162.68)` | `oklch(10% 0.030 162.68)` | Text on primary |

### Semantic Colors

| Token | Light Mode | Dark Mode | Usage |
|-------|-----------|-----------|-------|
| `--background` | `oklch(1 0 0)` | `oklch(0.145 0 0)` | Page background |
| `--foreground` | `oklch(0.145 0 0)` | `oklch(0.985 0 0)` | Primary text |
| `--muted` | `oklch(0.97 0 0)` | `oklch(0.269 0 0)` | Muted backgrounds |
| `--muted-foreground` | `oklch(0.556 0 0)` | `oklch(0.708 0 0)` | Secondary text |
| `--border` | `oklch(0.922 0 0)` | `oklch(1 0 0 / 10%)` | Borders |
| `--destructive` | `oklch(0.577 0.245 27.325)` | `oklch(0.704 0.191 22.216)` | Error states |

### Chart Colors

CSS variables for chart theming (supports light/dark modes):
- `--chart-1` through `--chart-5` - Pre-defined chart colors

### Color Scales

Full color scales available: `amber`, `neutral`, `green`, `teal`

```css
color: var(--color-teal-500);
background: var(--color-neutral-100);
```

---

## Typography

### Font Families

| Usage | Font |
|-------|------|
| Headings | Inconsolata |
| Body | Open Sans |
| Code | System monospace |

### Usage

```css
/* Applied automatically via index.css */
h1, h2, h3, h4, h5, h6 {
  font-family: "Inconsolata", sans-serif;
}

body {
  font-family: "Open Sans", sans-serif;
}
```

---

## Spacing & Radius

| Token | Value | Usage |
|-------|-------|-------|
| `--radius` | `0.625rem` (10px) | Default border radius |
| `--radius-sm` | `calc(var(--radius) - 4px)` | Small elements |
| `--radius-md` | `calc(var(--radius) - 2px)` | Medium elements |
| `--radius-lg` | `var(--radius)` | Large elements |
| `--radius-xl` | `calc(var(--radius) + 4px)` | Extra large elements |

---

## Effects

### Glass Effects

```css
.glass {
  backdrop-filter: blur(20px);
  background: var(--glass-bg);
  border: 1px solid var(--glass-border);
  box-shadow: var(--shadow-glass);
}
```

### Shadows

| Token | Usage |
|-------|-------|
| `--shadow-glass` | Glassmorphism shadow |
| `--shadow-elevated` | Elevated card shadow |
| `--shadow-glow` | Primary color glow |

### Gradients

| Token | Usage |
|-------|-------|
| `--gradient-primary` | Primary gradient (teal) |
| `--gradient-secondary` | Secondary gradient (subtle) |

---

## Tailwind Integration

All tokens are exposed to Tailwind via `@theme inline` in `index.css`:

```tsx
// Use semantic colors
<div className="bg-background text-foreground" />
<div className="bg-primary text-primary-foreground" />
<div className="border-border" />

// Use color scales
<div className="bg-teal-500" />
<div className="text-neutral-600" />
```

---

## Resources

- **ShadCN Components**: https://ui.shadcn.com/docs/components
- **ShadCN Blocks**: https://ui.shadcn.com/blocks
- **ShadCN Charts**: https://ui.shadcn.com/charts
- **Magic UI Components**: https://magicui.design/docs/components
