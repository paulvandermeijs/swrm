use std::collections::HashMap;

use gpui::{
    AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Window, div,
};
use gpui_component::{
    Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    setting::{SettingGroup, SettingItem, SettingPage, Settings},
    v_flex,
};
use swrm::app_state::AppState;
use swrm::settings::{MoveDir, SettingsEvent};

pub struct SettingsView {
    state: Entity<AppState>,
    inputs: HashMap<String, AgentInputs>,
    focus: FocusHandle,
    _store_sub: Subscription,
}

impl SettingsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = state.read(cx).settings.clone();
        let store_sub = cx.subscribe(&store, |_this, _store, event, cx| {
            if matches!(event, SettingsEvent::Changed) {
                cx.notify();
            }
        });
        let mut view = Self {
            state,
            inputs: HashMap::new(),
            focus: cx.focus_handle(),
            _store_sub: store_sub,
        };
        view.reconcile_inputs_with_window(window, cx);
        view
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Re-sync inputs on every render so newly-added agents get inputs.
        self.reconcile_inputs_with_window(window, cx);

        let agents = self.state.read(cx).settings.read(cx).agents().to_vec();
        let store = self.state.read(cx).settings.clone();
        let weak = cx.entity().downgrade();

        let mut items: Vec<SettingItem> = Vec::with_capacity(agents.len() + 1);
        let len = agents.len();
        for (idx, agent) in agents.iter().enumerate() {
            let id = agent.id.clone();
            let Some(input) = self.inputs.get(&id) else {
                continue;
            };
            let name_input = input.name.clone();
            let command_input = input.command.clone();
            let is_first = idx == 0;
            let is_last = idx + 1 == len;

            let store_for_up = store.clone();
            let id_up = id.clone();
            let store_for_down = store.clone();
            let id_down = id.clone();
            let store_for_del = store.clone();
            let id_del = id.clone();

            let btn_up_id: SharedString = format!("agent-up-{}", id).into();
            let btn_down_id: SharedString = format!("agent-down-{}", id).into();
            let btn_del_id: SharedString = format!("agent-del-{}", id).into();

            items.push(SettingItem::render(move |_opts, _window, _cx| {
                v_flex()
                    .gap_2()
                    .child(
                        h_flex().justify_between().child(Label::new("Agent")).child(
                            h_flex()
                                .gap_1()
                                .child(
                                    Button::new(btn_up_id.clone())
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::ArrowUp)
                                        .disabled(is_first)
                                        .on_click({
                                            let store = store_for_up.clone();
                                            let id = id_up.clone();
                                            move |_, _, cx| {
                                                store.update(cx, |s, cx| {
                                                    s.move_agent(&id, MoveDir::Up, cx);
                                                });
                                            }
                                        }),
                                )
                                .child(
                                    Button::new(btn_down_id.clone())
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::ArrowDown)
                                        .disabled(is_last)
                                        .on_click({
                                            let store = store_for_down.clone();
                                            let id = id_down.clone();
                                            move |_, _, cx| {
                                                store.update(cx, |s, cx| {
                                                    s.move_agent(&id, MoveDir::Down, cx);
                                                });
                                            }
                                        }),
                                )
                                .child(
                                    Button::new(btn_del_id.clone())
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Delete)
                                        .on_click({
                                            let store = store_for_del.clone();
                                            let id = id_del.clone();
                                            move |_, _, cx| {
                                                store.update(cx, |s, cx| {
                                                    s.remove_agent(&id, cx);
                                                });
                                            }
                                        }),
                                ),
                        ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Label::new("Name"))
                            .child(Input::new(&name_input)),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Label::new("Command"))
                            .child(Input::new(&command_input)),
                    )
                    .into_any_element()
            }));
        }

        // "+ Add agent" item.
        let store_for_add = store.clone();
        let weak_for_add = weak.clone();
        items.push(SettingItem::render(move |_opts, _window, _cx| {
            Button::new("add-agent")
                .primary()
                .label("+ Add agent")
                .on_click({
                    let store = store_for_add.clone();
                    let weak = weak_for_add.clone();
                    move |_, window, cx| {
                        store.update(cx, |s, cx| {
                            s.add_agent(cx);
                        });
                        let _ = weak.update(cx, |this, cx| {
                            this.reconcile_inputs_with_window(window, cx);
                            cx.notify();
                        });
                    }
                })
                .into_any_element()
        }));

        div()
            .track_focus(&self.focus)
            .size_full()
            .child(Settings::new("agents-settings").pages(vec![
                SettingPage::new("Agents").groups(vec![SettingGroup::new().items(items)]),
            ]))
    }
}

struct AgentInputs {
    name: Entity<InputState>,
    command: Entity<InputState>,
    _subs: Vec<Subscription>,
}

impl SettingsView {
    fn reconcile_inputs_with_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let agents = self.state.read(cx).settings.read(cx).agents().to_vec();
        let live_ids: std::collections::HashSet<String> =
            agents.iter().map(|a| a.id.clone()).collect();
        self.inputs.retain(|id, _| live_ids.contains(id));

        for agent in &agents {
            if self.inputs.contains_key(&agent.id) {
                continue;
            }
            let agent_name = agent.name.clone();
            let name_state = cx.new(|cx| InputState::new(window, cx).placeholder("agent name"));
            name_state.update(cx, |s, cx| {
                s.set_value(agent_name, window, cx);
            });
            let agent_command = agent.command.clone();
            let command_state =
                cx.new(|cx| InputState::new(window, cx).placeholder("shell command"));
            command_state.update(cx, |s, cx| {
                s.set_value(agent_command, window, cx);
            });
            let id_for_name = agent.id.clone();
            let name_sub = cx.subscribe(&name_state, move |this, _, event, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    this.commit_agent(&id_for_name, cx);
                }
            });
            let id_for_cmd = agent.id.clone();
            let command_sub = cx.subscribe(&command_state, move |this, _, event, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    this.commit_agent(&id_for_cmd, cx);
                }
            });
            self.inputs.insert(
                agent.id.clone(),
                AgentInputs {
                    name: name_state,
                    command: command_state,
                    _subs: vec![name_sub, command_sub],
                },
            );
        }
    }

    fn commit_agent(&self, id: &str, cx: &mut Context<Self>) {
        let Some(input) = self.inputs.get(id) else {
            return;
        };
        let name = input.name.read(cx).value().to_string();
        let command = input.command.read(cx).value().to_string();
        let store = self.state.read(cx).settings.clone();
        store.update(cx, |store, cx| {
            store.update_agent(id, name, command, cx);
        });
    }
}
