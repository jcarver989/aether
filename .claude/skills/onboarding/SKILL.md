---
name: onboarding
description: User onboarding UX patterns and best practices. Use when designing or implementing onboarding flows, first-run experiences, or user activation features.
allowed-tools:
  - Read
  - Glob
  - Grep
---

# Onboarding UX Best Practices

This skill provides patterns and guidelines for creating effective user onboarding experiences based on industry best practices from UserOnboard.

---

## Core Principle: Value Before Friction

The fundamental goal of onboarding is to help users reach their "aha moment" as quickly as possible. Every step should either deliver value or be necessary to enable value delivery.

**Key Question:** Is the user *actually better off* for having completed this step? Would they want to switch places with their future self on the other side?

---

## Pattern 1: Front-Loaded User Value

### What It Is
Ordering onboarding steps to deliver quick wins early, building momentum that carries users through the rest of the flow.

### The Psychology
- **Momentum is built in wins** — Two consecutive wins create a "hot streak" where tasks feel easier
- **Losing momentum is worse than no momentum** — Oscillating between wins and losses exponentially increases frustration
- **Success breeds success** — Get users to value fast and keep the wins coming

### Implementation Guidelines

1. **Audit your first 3-5 steps** — How many deliver actual user value vs. collect data for your benefit?

2. **Deliver value before requiring signup** (Gradual Engagement)
   - Let users experience core value before creating an account
   - Example: Duolingo teaches real language phrases before asking for registration

3. **Distinguish activity from achievement**
   - Customizing a template ≠ accomplishment
   - Actually using the product to solve a problem = accomplishment

4. **Difficulty is okay, complexity is not**
   - Users can handle challenging tasks if the mechanics are simple
   - Make instructions clear and easy to grasp quickly

### Examples
- **Duolingo**: Users learn real Swedish sentences before signing up
- **Dropbox**: Immediately syncs photos already on user's phone
- **IFTTT**: Provides pre-made, tested automation recipes
- **Slack**: Uses Slackbot to teach Slack by having users actually use Slack

---

## Pattern 2: Permission Priming

### What It Is
Preparing users before asking for system permissions to increase acceptance rates.

### Why It Matters
- You can only ask once — "Don't Allow" requires navigating to Settings to undo
- First interactions haven't built trust yet
- Access may be required to deliver core value

### Implementation Guidelines

1. **Direct awareness before the ask**
   - Users know a photo app needs camera access
   - A reminder prepares them better than a surprise dialog

2. **Tie access to value**
   - Explain *why* you're asking
   - Show *how* it benefits them

3. **Bake priming into the product**
   - Make the ask feel natural and integrated
   - Example: Dropbox's photo sync prompt looks like part of the interface

4. **Ask in context**
   - Request camera access when user opens the camera
   - Don't front-load permission requests at launch

### Priming Formula
```
[Benefit statement] + [Why you need access] + [User-controlled action to proceed]
```

### Examples
- **Instagram**: Asks for camera/photos only after user opens the camera
- **PayPal**: Creates a full screen state that explains notification benefits
- **Headspace**: Uses a seamless modal between interface and system dialog
- **Foursquare**: Shows the benefit of location access behind the system dialog

---

## Pattern 3: Success States

### What They Are
Positive feedback that confirms actions, provides context, or celebrates achievements — the opposite of error states.

### Three Types of Success States

#### 1. Confirmation States
**Purpose:** Answer user questions like "Did this work?" or "Am I doing this right?"

**When to Use:**
- After form submissions
- After completing setup steps
- When validating input

**Guidelines:**
- Make them visual (green = success is universally understood)
- Make them immediate (don't wait to show validation errors)
- Treat it as a conversation — user acts, product responds

**Anti-pattern:** Skype asks for password, shows no confirmation, then rejects it two steps later.

#### 2. Context States
**Purpose:** Situate users and direct them to the next step — like signposts.

**When to Use:**
- At branching points in the flow
- In long onboarding sequences
- When users need to know what they've accomplished

**Guidelines:**
- Show what's been accomplished and what's next
- Use a single, clear CTA
- Ensure the next step is relevant to this user

**Example:** Unroll.me shows "inbox zero" state with stats and suggests sharing while waiting for new emails.

#### 3. Encouragement States
**Purpose:** Celebrate meaningful accomplishments from the user's perspective.

**When to Use:**
- After milestones meaningful to users (not just to your metrics)
- When users complete core product actions

**Guidelines:**
- Save celebration for real milestones — empty praise erodes motivation
- Combine with context states to celebrate AND guide

**Good Example:** WordPress celebrates creating your site (why users signed up)
**Anti-pattern:** Vimeo celebrates signing up (only meaningful to the product, not the user)

---

## Pattern 4: Progression Systems

### What They Are
To-do lists, completion meters, or progress trackers that guide users from "completely new" to "fully capable."

### The Progress Quadrant

Rate your progression system on two axes:
- **X-axis (User Progress):** Does completion lead to real-world outcomes?
- **Y-axis (Path Clarity):** Are the steps clear and well-broken-down?

**Quadrant 4 (High/High):** Winner — clear steps leading to real outcomes
**Quadrants 1 & 2 (Low progress):** Users aren't making real progress, bigger problem
**Quadrant 3 (Low clarity, high progress):** Rare case where users progress without explicit steps

### Implementation Guidelines

1. **Situate users**
   - Clear visual states for: "you are here", "you've been there", "go here next"
   - New users can't navigate comfortably without guidance

2. **Set expectations**
   - Show how much effort is required upfront
   - Showcase the process so users know what's coming

3. **Balance explanation with action**
   - Start with easy, familiar steps
   - Progress to new, unfamiliar ones

4. **Be honest about effort**
   - No hidden steps-within-steps
   - No unexpected wait times
   - Update progress immediately after step completion

5. **Make it visually prominent**
   - Don't let it blend into the interface
   - Use icons, words, arrows, and colors

### Examples
- **Zendesk**: Progress system ends with solving real support tickets
- **Waze**: Gamified system tracking real distance driven
- **WordPress**: Long but parseable steps with time estimates

---

## Quick Reference: Onboarding Checklist

### Before Building
- [ ] What is the user's "aha moment"? What's the shortest path there?
- [ ] What value can be delivered before requiring signup?
- [ ] What permissions are required? When is the right context to ask?

### Flow Design
- [ ] First 3 steps deliver user value (not just collect data)
- [ ] Permissions are requested in context, with priming
- [ ] Success states confirm actions immediately
- [ ] Progress is visually tracked with clear next steps

### Each Step Audit
- [ ] Is this step necessary to deliver value?
- [ ] Can this step be postponed until after value delivery?
- [ ] Does completing this step make the user actually better off?

### Post-Completion
- [ ] Celebrate meaningful user accomplishments (not product metrics)
- [ ] Provide clear path to next valuable action
- [ ] Don't celebrate prematurely (signing up ≠ accomplishment)

---

## Anti-Patterns to Avoid

1. **Front-loading friction** — Asking for signup/permissions before showing value
2. **Celebrating activity** — Praising steps that only benefit the product
3. **Hidden complexity** — Steps that contain sub-steps or unexpected waits
4. **Permission ambush** — Requesting access without explanation or context
5. **Static progress** — Trackers that don't update after completion
6. **Competing CTAs** — Multiple next-step suggestions at decision points
7. **Premature celebration** — Congratulating signup instead of real achievement
