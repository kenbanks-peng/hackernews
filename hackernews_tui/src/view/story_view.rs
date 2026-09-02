use super::{article_view, comment_view, help_view::HasHelpView, text_view, traits::*, utils};
use crate::client::StoryNumericFilters;
use crate::prelude::*;

static STORY_TAGS: [&str; 5] = ["front_page", "story", "ask_hn", "show_hn", "job"];

/// StoryView is a View displaying a list stories corresponding
/// to a particular category (top stories, newest stories, most popular stories, etc).
pub struct StoryView {
    pub stories: Vec<Story>,

    view: ScrollView<LinearLayout>,
    raw_command: String,
    starting_id: usize,
    story_receiver: Option<std::sync::mpsc::Receiver<Story>>,
    has_placeholder: bool,
}

impl ViewWrapper for StoryView {
    wrap_impl!(self.view: ScrollView<LinearLayout>);

    fn wrap_layout(&mut self, size: Vec2) {
        self.try_append_story();
        self.view.layout(size);
    }
}

impl StoryView {
    pub fn new(stories: Vec<Story>, starting_id: usize) -> Self {
        StoryView {
            view: Self::construct_story_view(&stories, starting_id),
            stories,
            raw_command: String::new(),
            starting_id,
            story_receiver: None,
            has_placeholder: false,
        }
    }

    pub fn new_streaming(
        starting_id: usize,
        story_receiver: std::sync::mpsc::Receiver<Story>,
    ) -> Self {
        StoryView {
            view: LinearLayout::vertical()
                .child(text_view::TextView::new(
                    "Scanning for stories with 500+ points…",
                ))
                .scrollable(),
            stories: vec![],
            raw_command: String::new(),
            starting_id,
            story_receiver: Some(story_receiver),
            has_placeholder: true,
        }
    }

    fn max_id_len(starting_id: usize) -> usize {
        (starting_id + client::STORY_LIMIT).to_string().len()
    }

    fn construct_story_item(story: &Story, id: usize, max_id_len: usize) -> text_view::TextView {
        let mut story_text = StyledString::styled(
            format!("{1:>0$}. ", max_id_len, id),
            config::get_config_theme().component_style.metadata,
        );
        story_text.append(Self::get_story_text(max_id_len, story));
        text_view::TextView::new(story_text)
    }

    fn construct_story_view(stories: &[Story], starting_id: usize) -> ScrollView<LinearLayout> {
        let max_id_len = Self::max_id_len(starting_id);

        LinearLayout::vertical()
            .with(|view| {
                stories.iter().enumerate().for_each(|(i, story)| {
                    view.add_child(Self::construct_story_item(
                        story,
                        starting_id + i + 1,
                        max_id_len,
                    ));
                })
            })
            .scrollable()
    }

    /// Append downloaded stories without replacing the view or disturbing its focus.
    fn try_append_story(&mut self) {
        let mut downloaded_stories = vec![];
        let mut disconnected = false;
        if let Some(receiver) = &self.story_receiver {
            loop {
                match receiver.try_recv() {
                    Ok(story) => downloaded_stories.push(story),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }

        if disconnected {
            self.story_receiver = None;
            if self.stories.is_empty() && downloaded_stories.is_empty() {
                if let Some(view) = self
                    .get_item_mut(0)
                    .and_then(|view| view.downcast_mut::<text_view::TextView>())
                {
                    view.set_content("No stories with 500+ points on this page.");
                }
            }
        }
        if downloaded_stories.is_empty() {
            return;
        }

        if self.has_placeholder {
            self.get_inner_list_mut().remove_child(0);
            self.has_placeholder = false;
        }
        for story in downloaded_stories {
            let id = self.starting_id + self.stories.len() + 1;
            self.add_item(Self::construct_story_item(
                &story,
                id,
                Self::max_id_len(self.starting_id),
            ));
            self.stories.push(story);
        }
    }

    /// Get the text summarizing basic information about a story
    fn get_story_text(max_id_len: usize, story: &Story) -> StyledString {
        let mut story_text = story.styled_title();

        if let Ok(url) = url::Url::parse(&story.url) {
            if let Some(domain) = url.domain() {
                story_text.append_styled(
                    format!(" ({domain})"),
                    config::get_config_theme().component_style.link,
                );
            }
        }

        story_text.append_plain("\n");

        story_text.append_styled(
            // left-align the story's metadata by `max_id_len+2`,
            // which is the maximum width of a string `{story_id}. `
            format!(
                "{:width$}{} points | by {} | {} ago | {} comments",
                " ",
                story.points,
                story.author,
                crate::utils::get_elapsed_time_as_text(story.time),
                story.num_comments,
                width = max_id_len + 2,
            ),
            config::get_config_theme().component_style.metadata,
        );
        story_text
    }

    inner_getters!(self.view: ScrollView<LinearLayout>);
}

impl ListViewContainer for StoryView {
    fn get_inner_list(&self) -> &LinearLayout {
        self.get_inner().get_inner()
    }

    fn get_inner_list_mut(&mut self) -> &mut LinearLayout {
        self.get_inner_mut().get_inner_mut()
    }

    fn on_set_focus_index(&mut self, old_id: usize, new_id: usize) {
        let direction = old_id <= new_id;

        // enable auto-scrolling when changing the focused index of the view
        self.scroll(direction);
    }
}

impl ScrollViewContainer for StoryView {
    type ScrollInner = LinearLayout;

    fn get_inner_scroll_view(&self) -> &ScrollView<LinearLayout> {
        self.get_inner()
    }

    fn get_inner_scroll_view_mut(&mut self) -> &mut ScrollView<LinearLayout> {
        self.get_inner_mut()
    }
}

pub fn construct_story_main_view(
    stories: Vec<Story>,
    client: &'static client::HNClient,
    starting_id: usize,
) -> OnEventView<StoryView> {
    construct_story_main_view_from(StoryView::new(stories, starting_id), client, starting_id)
}

fn construct_story_main_view_from(
    story_view: StoryView,
    client: &'static client::HNClient,
    starting_id: usize,
) -> OnEventView<StoryView> {
    let is_suffix_key =
        |c: &Event| -> bool { config::get_story_view_keymap().goto_story.has_event(c) };

    let story_view_keymap = config::get_story_view_keymap().clone();

    OnEventView::new(story_view)
        // number parsing
        .on_pre_event_inner(EventTrigger::from_fn(|_| true), move |s, e| {
            match *e {
                Event::Char(c) if c.is_ascii_digit() => {
                    s.raw_command.push(c);
                }
                _ => {
                    if !is_suffix_key(e) {
                        s.raw_command.clear();
                    }
                }
            };

            // don't allow the inner `LinearLayout` child view to handle the event
            // because of its pre-defined `on_event` function
            Some(EventResult::Ignored)
        })
        // story navigation shortcuts
        .on_pre_event_inner(story_view_keymap.prev_story, |s, _| {
            if s.stories.is_empty() {
                return None;
            }
            let id = s.get_focus_index();
            if id == 0 {
                None
            } else {
                s.set_focus_index(id - 1)
            }
        })
        .on_pre_event_inner(story_view_keymap.next_story, |s, _| {
            if s.stories.is_empty() {
                return None;
            }
            let id = s.get_focus_index();
            s.set_focus_index(id + 1)
        })
        .on_pre_event_inner(story_view_keymap.goto_story_comment_view, move |s, _| {
            let id = s.get_focus_index();
            // the story struct hasn't had any comments inside yet,
            // so it can be cloned without greatly affecting performance
            let item_id = s.stories.get(id)?.id;
            Some(EventResult::with_cb({
                move |s| comment_view::construct_and_add_new_comment_view(s, client, item_id, false)
            }))
        })
        // open external link shortcuts
        .on_pre_event_inner(story_view_keymap.open_article_in_browser, move |s, _| {
            let id = s.get_focus_index();
            utils::open_url_in_browser(s.stories.get(id)?.get_url().as_ref());
            Some(EventResult::Consumed(None))
        })
        .on_pre_event_inner(
            story_view_keymap.open_article_in_article_view,
            move |s, _| {
                let id = s.get_focus_index();
                let url = s.stories.get(id)?.url.clone();
                if !url.is_empty() {
                    Some(EventResult::with_cb({
                        move |s| article_view::construct_and_add_new_article_view(client, s, &url)
                    }))
                } else {
                    Some(EventResult::Consumed(None))
                }
            },
        )
        .on_pre_event_inner(story_view_keymap.open_story_in_browser, move |s, _| {
            let url = s.stories.get(s.get_focus_index())?.story_url();
            utils::open_url_in_browser(&url);
            Some(EventResult::Consumed(None))
        })
        .on_pre_event_inner(story_view_keymap.goto_story, move |s, _| {
            match s.raw_command.parse::<usize>() {
                Ok(number) => {
                    s.raw_command.clear();
                    if number < starting_id + 1 {
                        return None;
                    }
                    let number = number - 1 - starting_id;
                    if number < s.len() {
                        s.set_focus_index(number).unwrap();
                        Some(EventResult::Consumed(None))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        })
        .on_scroll_events()
}

fn get_story_view_title_bar(tag: &'static str, sort_mode: client::StorySortMode) -> impl View {
    let style = config::get_config_theme().component_style.title_bar;
    let mut title = StyledString::styled(
        "[Y]",
        Style::from(style).combine(ColorStyle::front(
            config::get_config_theme().palette.light_white,
        )),
    );
    title.append_styled(" Hacker News (500+ points)", style);

    for (i, item) in STORY_TAGS.iter().enumerate() {
        title.append_styled(" | ", style);
        if *item == tag {
            let sort_mode_desc = match sort_mode {
                client::StorySortMode::None => "",
                client::StorySortMode::Date => " (by_date)",
                client::StorySortMode::Points => " (by_point)",
            };
            title.append_styled(
                format!("{}.{}{}", i + 1, item, sort_mode_desc),
                Style::from(style)
                    .combine(config::get_config_theme().component_style.current_story_tag),
            );
        } else {
            title.append_styled(format!("{}.{}", i + 1, item), style);
        }
    }
    title.append_styled(" | ", style);

    PaddedView::lrtb(
        0,
        0,
        0,
        1,
        Layer::with_color(TextView::new(title), style.into()),
    )
}

/// Construct a story view given a list of stories.
pub fn construct_story_view(
    stories: Vec<Story>,
    client: &'static client::HNClient,
    tag: &'static str,
    sort_mode: client::StorySortMode,
    page: usize,
    numeric_filters: client::StoryNumericFilters,
) -> impl View {
    let starting_id = client::STORY_LIMIT * page;
    construct_story_view_from_main(
        construct_story_main_view(stories, client, starting_id),
        client,
        tag,
        sort_mode,
        page,
        numeric_filters,
    )
}

fn construct_story_view_from_main(
    main_view: OnEventView<StoryView>,
    client: &'static client::HNClient,
    tag: &'static str,
    sort_mode: client::StorySortMode,
    page: usize,
    numeric_filters: client::StoryNumericFilters,
) -> impl View {
    let mut view = LinearLayout::vertical()
        .child(get_story_view_title_bar(tag, sort_mode))
        .child(main_view.full_height())
        .child(utils::construct_footer_view::<StoryView>());
    view.set_focus_index(1)
        .unwrap_or(EventResult::Consumed(None));

    let current_tag_pos = STORY_TAGS
        .iter()
        .position(|t| *t == tag)
        .unwrap_or_else(|| panic!("unkwnown tag {tag}"));

    let story_view_keymap = config::get_story_view_keymap().clone();

    // Because we re-use the story main view to construct a search view,
    // some of the story keymaps need to be handled here instead of by the main view like
    // for comment views or article views.

    OnEventView::new(view)
        .on_pre_event(config::get_global_keymap().open_help_dialog.clone(), |s| {
            s.add_layer(StoryView::construct_on_event_help_view())
        })
        .on_pre_event(story_view_keymap.cycle_sort_mode, move |s| {
            // disable "search_by_date" for front_page stories
            if tag == "front_page" {
                return;
            }
            construct_and_add_new_story_view(
                s,
                client,
                tag,
                sort_mode.next(tag),
                0,
                numeric_filters,
                true,
            );
        })
        // story tag navigation
        .on_pre_event(story_view_keymap.next_story_tag, move |s| {
            let next_tag = STORY_TAGS[(current_tag_pos + 1) % STORY_TAGS.len()];
            construct_and_add_new_story_view(
                s,
                client,
                next_tag,
                if next_tag == "story" || next_tag == "job" {
                    client::StorySortMode::Date
                } else {
                    client::StorySortMode::None
                },
                0,
                StoryNumericFilters::default(),
                false,
            );
        })
        .on_pre_event(story_view_keymap.prev_story_tag, move |s| {
            let prev_tag = STORY_TAGS[(current_tag_pos + STORY_TAGS.len() - 1) % STORY_TAGS.len()];
            construct_and_add_new_story_view(
                s,
                client,
                prev_tag,
                if prev_tag == "story" || prev_tag == "job" {
                    client::StorySortMode::Date
                } else {
                    client::StorySortMode::None
                },
                0,
                StoryNumericFilters::default(),
                false,
            );
        })
        // paging
        .on_pre_event(story_view_keymap.prev_page, move |s| {
            if page > 0 {
                construct_and_add_new_story_view(
                    s,
                    client,
                    tag,
                    sort_mode,
                    page - 1,
                    numeric_filters,
                    true,
                );
            }
        })
        .on_pre_event(story_view_keymap.next_page, move |s| {
            construct_and_add_new_story_view(
                s,
                client,
                tag,
                sort_mode,
                page + 1,
                numeric_filters,
                true,
            );
        })
}

/// Retrieve a list of stories satisfying some conditions and construct a story view displaying them.
pub fn construct_and_add_new_story_view(
    s: &mut Cursive,
    client: &'static client::HNClient,
    tag: &'static str,
    sort_mode: client::StorySortMode,
    page: usize,
    numeric_filters: client::StoryNumericFilters,
    pop_layer: bool,
) {
    let (sender, receiver) = std::sync::mpsc::channel();
    let cb_sink = s.cb_sink().clone();

    std::thread::spawn(move || {
        let result = client.stream_stories_by_tag(tag, sort_mode, page, numeric_filters, |story| {
            if sender.send(story).is_err() {
                return false;
            }
            cb_sink.send(Box::new(|_| {})).is_ok()
        });
        if let Err(err) = result {
            warn!(
                "failed to scan stories (tag={tag}, sort_mode={sort_mode:?}, page={page}): {err}"
            );
        }
        // Wake the renderer once more so it observes channel disconnection and
        // can replace the scanning placeholder when no stories were found.
        let _ = cb_sink.send(Box::new(|_| {}));
    });

    let starting_id = client::STORY_LIMIT * page;
    let main_view = construct_story_main_view_from(
        StoryView::new_streaming(starting_id, receiver),
        client,
        starting_id,
    );
    let story_view =
        construct_story_view_from_main(main_view, client, tag, sort_mode, page, numeric_filters);

    if pop_layer {
        s.pop_layer();
    }
    s.screen_mut().add_transparent_layer(Layer::new(story_view));
}
