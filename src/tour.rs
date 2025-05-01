use std::path::{Path, PathBuf};

use bytes::Bytes;
use iced_widget::button::Style;
use iced_widget::core::{Background, Color, ContentFit, Length, Padding, Shadow, border};
use iced_widget::runtime::{Program, Task};
use iced_widget::{button, column, container, image as iced_image, row, scrollable, slider, text};
use image::{EncodableLayout, RgbaImage};

use crate::Element;

type Font = <iced_widget::renderer::Renderer as iced_widget::core::text::Renderer>::Font;

#[derive(Debug, Clone)]
struct ImageInfo {
    width: u32,
    height: u32,
    image: RgbaImage,
    bytes: Bytes,
    name: String,
    zoom: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FolderItem {
    name: String,
    is_dir: bool,
}

impl Ord for FolderItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self.is_dir, other.is_dir) {
            (true, true) | (false, false) => self.name.cmp(&other.name),
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
        }
    }
}
impl PartialOrd for FolderItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone)]
enum State {
    ChoosingImage {
        folder: PathBuf,
        items: Vec<FolderItem>,
        message: Option<String>,
    },
    ViewingImage(ImageInfo),
}

#[derive(Debug)]
pub struct Tour {
    state: State,
}

impl Program for Tour {
    type Message = Message;
    type Renderer = iced_widget::Renderer;
    type Theme = iced_widget::Theme;

    fn update(&mut self, event: Self::Message) -> Task<Message> {
        self.update(event)
    }

    fn view(&self) -> Element<Self::Message> {
        self.view()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    ImageClosed,
    SubfolderSelected(String),
    SubfolderUp,
    ImageSelected(String),
    ZoomChanged(f32),
}

fn list_folder(folder: &Path) -> Vec<FolderItem> {
    let items = match std::fs::read_dir(folder) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Error reading directory: {e}");
            return vec![];
        }
    };

    let mut items: Vec<FolderItem> = items
        .filter_map(|entry| {
            let entry = entry.unwrap();
            let file_type = entry.file_type().unwrap();
            let is_dir = if file_type.is_dir() {
                true
            } else if file_type.is_file() {
                false
            } else {
                return None;
            };
            Some(FolderItem {
                name: entry.file_name().into_string().unwrap(),
                is_dir,
            })
        })
        .collect();
    items.sort();
    items
}

fn load_image(path: &Path) -> Result<ImageInfo, String> {
    let name = path.file_name().map_or_else(
        || "Error opening file: missing filename".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let image = std::fs::read(path).map_err(|e| format!("Error reading image: {e}"))?;
    let image = image::load_from_memory(&image).map_err(|e| format!("error parsing image: {e}"))?;
    let image = image.into_rgba8();
    Ok(ImageInfo {
        width: image.width(),
        height: image.height(),
        bytes: Bytes::copy_from_slice(image.as_bytes()),
        image,
        zoom: 1.0,
        name,
    })
}

const DEFAULT_MSG: &'static str = "Please select an image";

impl Tour {
    fn update(&mut self, event: Message) -> Task<Message> {
        if let State::ChoosingImage { message, .. } = &mut self.state {
            *message = None;
        }

        match (event, &mut self.state) {
            (
                Message::ImageSelected(name),
                State::ChoosingImage {
                    folder, message, ..
                },
            ) => match load_image(&folder.join(&name)) {
                Err(e) => {
                    *message = Some(e);
                }
                Ok(image) => {
                    self.state = State::ViewingImage(image);
                }
            },
            (Message::SubfolderSelected(subfolder), State::ChoosingImage { folder, items, .. }) => {
                folder.push(subfolder);
                *items = list_folder(folder);
            }
            (Message::ImageClosed, State::ViewingImage { .. }) => {
                let mut folder = PathBuf::new();
                folder.push("/");
                self.state = State::ChoosingImage {
                    items: list_folder(&folder),
                    folder,
                    message: None,
                };
            }
            (Message::SubfolderUp, State::ChoosingImage { folder, items, .. }) => {
                folder.pop();
                *items = list_folder(folder);
            }
            (Message::ZoomChanged(z), State::ViewingImage(img)) => {
                img.zoom = z;
                img.bytes = image::imageops::resize(
                    &img.image,
                    (img.width as f32 * z) as u32,
                    (img.height as f32 * z) as u32,
                    image::imageops::FilterType::Lanczos3,
                )
                .into_raw()
                .into();
            }
            x => {
                eprintln!("Incorrect message: {x:?}");
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<Message> {
        match &self.state {
            State::ChoosingImage {
                folder,
                items,
                message,
            } => self.image_chooser(folder, items, message.clone()),
            State::ViewingImage(img) => self.image_viewer(img),
        }
    }

    fn chooser_button(&self, label: String, msg: Message) -> Element<Message> {
        container(
            button(text(label).font(Font::MONOSPACE))
                .on_press(msg)
                .width(Length::Fill)
                .padding(Padding::new(3.0).left(10))
                .style(|_, status| Style {
                    background: match status {
                        button::Status::Hovered => {
                            Some(Background::Color(Color::from_rgb8(200, 200, 255)))
                        }
                        _ => None,
                    },
                    border: border::color(Color::BLACK).width(1),
                    text_color: Color::BLACK,
                    shadow: Shadow::default(),
                }),
        )
        .padding(Padding {
            left: 20.0,
            ..Default::default()
        })
        .into()
    }

    fn image_chooser(
        &self,
        folder: &Path,
        items: &[FolderItem],
        message: Option<String>,
    ) -> Element<Message> {
        let msg = match message {
            Some(msg) => text(msg).color(Color::from_rgb8(255, 0, 0)),
            None => text(DEFAULT_MSG),
        };
        column![
            msg,
            text(folder.to_string_lossy().into_owned()).font(Font::MONOSPACE),
            self.chooser_button("..".to_owned(), Message::SubfolderUp),
            scrollable(items.into_iter().fold(column([]), |column, entry| {
                let name = entry.name.clone();
                column.push(if entry.is_dir {
                    self.chooser_button(
                        format!("{name}/"),
                        Message::SubfolderSelected(name.clone()),
                    )
                } else {
                    self.chooser_button(name.clone(), Message::ImageSelected(name.clone()))
                })
            }))
        ]
        .padding(10.0)
        .into()
    }

    fn image_viewer(
        &self,
        ImageInfo {
            width,
            height,
            bytes,
            name,
            zoom,
            ..
        }: &ImageInfo,
    ) -> Element<Message> {
        let max_width = 2000.0;
        let max_height = 2000.0;
        let max_zoom = (10.0_f32)
            .minimum(max_width / *width as f32)
            .minimum(max_height / *height as f32);
        let header = row![
            text(name.to_owned()).font(Font::MONOSPACE),
            slider(0.1..=max_zoom, *zoom, |z| Message::ZoomChanged(z)).step(0.1),
            text(format!("Zoom: {:.1}x", zoom)).font(Font::MONOSPACE),
            button("Close image")
                .on_press(Message::ImageClosed)
                .padding(3.0),
        ]
        .padding(10.0)
        .spacing(5.0);
        let img = container(
            iced_image(iced_image::Handle::from_rgba(
                (*width as f32 * zoom) as u32,
                (*height as f32 * zoom) as u32,
                // this is a cheap copy
                bytes.clone(),
            ))
            .content_fit(ContentFit::None),
        )
        .center(Length::Fill);
        column![header, img,].into()
    }

    pub fn new(image: Option<impl AsRef<Path>>) -> Self {
        Self {
            state: match image {
                Some(image) => match load_image(image.as_ref()) {
                    Ok(image) => State::ViewingImage(image),
                    Err(e) => {
                        let folder = image.as_ref().parent().unwrap_or(Path::new("/"));
                        State::ChoosingImage {
                            folder: folder.to_path_buf(),
                            items: list_folder(folder),
                            message: Some(e),
                        }
                    }
                },
                None => {
                    let folder = PathBuf::from("/");
                    State::ChoosingImage {
                        items: list_folder(&folder),
                        folder,
                        message: None,
                    }
                }
            },
        }
    }
}
