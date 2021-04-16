
extern crate rouge;

use std::{
	fs::File, path::{Path, PathBuf}
};
use rouge::{Event, Widget, Layout, Data, Color, event::MouseInput, message};

#[derive(Debug, Clone, PartialEq)]
struct Gallery {
	location: PathBuf,
	index: usize,
	images: Vec<PathBuf>
}

impl Gallery {
	fn new<P>(path: P) -> Self
		where P: AsRef<Path> {
		Self {
			location: path.as_ref().into(),
			index: 0,
			images: path.as_ref()
				.read_dir()
				.map(|e| e.into_iter()
					.skip_while(|e| e.is_err())
					.map(|e| e.unwrap().path())
					.filter(|e| match e.extension().map(|e| e.to_str()).flatten() {
						Some(ext) => match ext {
							"bmp" | "ico" | "jpg" | "jpeg" | "gif" |
							"png" | "tiff" | "webp" | "avif" | "pnm" |
							"dds" | "tga" => true,
							_ => false
						},
						_ => false
					}))
				.map(|a| a.collect())
				.unwrap_or(Vec::new())
		}
	}
	
	fn move_to<P>(&mut self, path: P)
		where P: AsRef<Path> {
		match self.images.iter().position(|i| i == path.as_ref()) {
			Some(index) => {
				self.index = index;
			},
			_ => ()
		}
	}
	
	fn next(&mut self) -> Option<&PathBuf> {
		if self.index == self.images.len() - 1 {
			self.index = 0;
		} else {
			self.index += 1;
		}
		self.images.get(self.index)
	}
	
	fn prev(&mut self) -> Option<&PathBuf> {
		if self.index == 0 {
			self.index = self.images.len() - 1;
		} else {
			self.index -= 1;
		}
		self.images.get(self.index)
	}
	
	fn position(&self) -> usize {
		self.index
	}
	
	fn size(&self) -> usize {
		self.images.len()
	}
}

fn window_title(name: &str, size: (u32, u32)) -> String {
	format!("{} - ({} x {}) - Rim", name, size.0, size.1)
}

fn image_next(i: &mut rouge::Image) {
	let (path, size) = match i.data_mut().get_mut::<Gallery>("gallery")
		.map(|g| (g.next().map(|p| p.clone()), g.size())) {
		None => return,
		Some((p, s)) => (p, s)
	};
	
	if size > 1 {
		if let Some(path) = path {
			i.set_with_path(&path);
			if let Some(name) = path.file_name().map(|p| p.to_str()).flatten() {
				let data = message::Data::new(window_title(name, i.dimensions()));
				i.emit_with_data("title", data);
			}
			
			scale_image(i);
		}
	}
}

fn image_prev(i: &mut rouge::Image) {
	let (path, size) = match i.data_mut().get_mut::<Gallery>("gallery")
		.map(|g| (g.prev().map(|p| p.clone()), g.size())) {
		None => return,
		Some((p, s)) => (p, s)
	};
	
	if size > 1 {
		if let Some(path) = path {
			i.set_with_path(&path);
			if let Some(name) = path.file_name().map(|p| p.to_str()).flatten() {
				let data = message::Data::new(window_title(name, i.dimensions()));
				i.emit_with_data("title", data);
			}
			scale_image(i);
		}
	}
}

fn scale_image(i: &mut rouge::Image) {
	let scale = *i.data_mut().get_mut::<f32>("scale").unwrap();
	let xsf = i.source_width() as f32 / scale;
	let ysf = i.source_height() as f32 / scale;
	i.set_size((xsf as u32, ysf as u32));
	i.update();
}

const CURRENT_DIRECTORY: &str = ".";
const IMAGE_SCALE_DEFAULT: f32 = 1.0;
const IMAGE_SCALE_FACTOR: f32 = 1.2;
const IMAGE_SCALE_MIN: f32 = 32.0;
const IMAGE_SCALE_MAX: f32 = 0.00390625;

fn scale_image_default(i: &mut rouge::Image) {
	if i.dimensions() != i.source_dimensions() {
		*i.data_mut().get_mut::<f32>("scale").unwrap() = IMAGE_SCALE_DEFAULT;
		i.set_size(i.source_dimensions());
		i.update();
	}
}

fn scale_image_up(i: &mut rouge::Image) {
	let scale = i.data_mut().get_mut::<f32>("scale").unwrap();
	if *scale > IMAGE_SCALE_MAX {
		*scale /= IMAGE_SCALE_FACTOR;
		let sf = *scale;
		let xsf = i.source_width() as f32 / sf;
		let ysf = i.source_height() as f32 / sf;
		i.set_size((xsf as u32, ysf as u32));
		i.update();
	}
}

fn scale_image_down(i: &mut rouge::Image) {
	let scale = i.data_mut().get_mut::<f32>("scale").unwrap();
	if *scale < IMAGE_SCALE_MIN {
		*scale *= IMAGE_SCALE_FACTOR;
		let sf = *scale;
		let xsf = i.source_width() as f32 / sf;
		let ysf = i.source_height() as f32 / sf;
		i.set_size((xsf as u32, ysf as u32));
		i.update();
	}
}

fn main() {
	let args = std::env::args().collect::<Vec<_>>();
	let path = args.get(1);
	let (name, image) = match path {
		None => ("".into(), rouge::Image::new()),
		Some(path) => match File::open(path) {
			Err(_) => (path.into(), rouge::Image::new()),
			Ok(image) => {
				let path = Path::new(path);
				let directory = match path.parent() {
					None => Path::new(CURRENT_DIRECTORY),
					Some(path) => {
						if let Some(empty) = path.to_str().map(|s| s.is_empty()) {
							if empty {
								Path::new(CURRENT_DIRECTORY)
							} else {
								path
							}
						} else {
							path
						}
					}
				};
				let mut gallery = Gallery::new(directory);
				gallery.move_to(&path);
				let image = rouge::Image::from(image)
					.with_data(Data::new()
						.with("scale", IMAGE_SCALE_DEFAULT)
						.with("gallery", gallery)
						.with("grab", Option::<(i16, i16)>::None))
					.with_on_click(|i, e| {
						if let Some(MouseInput::Left) = e.mouse_input() {
							i.set_opacity(0.5);
							let data = Some(e.mouse().unwrap().position());
							*i.data_mut().get_mut::<Option<(i16, i16)>>("grab").unwrap() = data;
						}
					})
					.with_on_release(|i, e| {
						if let Some(MouseInput::Left) = e.mouse_input() {
							i.set_opacity(1.0);
							*i.data_mut().get_mut::<Option<(i16, i16)>>("grab").unwrap() = None;
						}
					})
					.with_on("next", |i, _| image_next(i))
					.with_on("prev", |i, _| image_prev(i))
					.with_on("resize", |i, e| {
						match e.mouse_input() {
							None => (),
							Some(input) => match input {
								MouseInput::Middle => {
									scale_image_default(i);
								},
								MouseInput::ScrollUp => {
									scale_image_up(i);
								},
								MouseInput::ScrollDown => {
									scale_image_down(i);
								},
								_ => ()
							}
						}
					})
					.with_on("keypress", move |i, e| {
						if let Some(key) = e.key() {
							match key.code() {
								19 | 90 => {
									scale_image_default(i);
								},
								21 | 86 => {
									scale_image_up(i);
								},
								20 | 82 => {
									scale_image_down(i);
								},
								// Left
								113 | 83 => {
									image_prev(i);
								},
								// Up
								111 | 80 => {},
								// Right
								114 | 85 => {
									image_next(i);
								},
								// Down
								116 | 88 => {},
								_ => ()
							}
						}
					});
				let name = path.file_name()
					.map(|n| n.to_str()).flatten()
					.unwrap_or("[filename]").to_string();
				(name, image)
			}
		}
	};
	
	let size = match image.dimensions() {
		(0, 0) => (320, 240),
		(a, b) => (a, b)
	};
	
	let title = if image.is_valid() { window_title(&name, size) } else { "Rim".into() };
	
	let context = ["Prev", "Next"].iter()
		.map(|s| rouge::Button::from(s)
			.with_font(rouge::Font::new(18.0, rouge::Color::Rgb(0xC1, 0xC1, 0xC1)))
			.without_border()
			.with_background_color(rouge::Color::Rgb(0x13, 0x16, 0x85))
			.with_on_click(|b, e| {
				if let Some(MouseInput::Left) = e.mouse_input() {
					b.emit(&b.get_text().to_lowercase());
				}
			}))
		.collect::<rouge::List>()
		.horizontal()
		.with_relay_bind(true);
	
	rouge::main(async {
		rouge::App::new()
			.window(rouge::Window::new()
				.title(title)
				.resize(size)
				.visible(true)
				.with_background_color(Color::Rgb(0x24, 0x25, 0x26))
				.with_on_click(|w, e| { w[0][0].trigger_context("resize", e); w.update();})
				.with_bind("title", |w, d| {
					if let Some(Ok(title)) = d.map(|d| d.into::<String>()) {
						w.set_title(title);
					}
				})
				.with_bind("next", |w, _| w[0][0].trigger("next"))
				.with_bind("prev", |w, _| w[0][0].trigger("prev"))
				.widget(rouge::Stack::new()
					.with_relay_bind(true)
					.widget(image)
					.widget(context)))
	});
}
