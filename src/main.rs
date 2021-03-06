
extern crate rouge;

use std::path::Path;
use rouge::{
	Event, Widget, WidgetBuilder, WidgetWrap, WidgetUnwrap, Layout, LayoutExt,
	Color, Align, Space, event::KeyMap, message::{Sender, WidgetChannelExt},
	common::{ScrollRegion, ScrollRegionBuilder}
};

fn get_image_paths<P: AsRef<Path>>(path: P) -> Vec<String> {
	use lexical_sort::{StringSort, natural_lexical_only_alnum_cmp as cmp};

	let paths = match path.as_ref().read_dir() {
		Ok(p) => p,
		_ => return Vec::new()
	};

	let mut paths = paths
		.into_iter()
		.filter_map(|e| e.ok().map(|e| e.path()))
		.filter(|e| match e.extension().map(|e| e.to_str()).flatten() {
			Some(ext) => match ext.to_lowercase().as_str() {
				"bmp" | "ico" | "jpg" | "jpeg" | "gif" |
				"png" | "tiff" | "webp" | "avif" | "pnm" |
				"dds" | "tga" => true,
				_ => false
			},
			_ => false
		})
		.filter_map(|e| e.to_str().map(|e| e.to_string()))
		.collect::<Vec<_>>();
	paths.string_sort_unstable(cmp);
	paths
}

#[derive(Debug, Clone, PartialEq)]
struct Gallery {
	index: usize,
	images: Vec<String>
}

impl Gallery {
	fn new<P>(path: P) -> Self
	where P: AsRef<Path> {
		let mut path = path.as_ref().to_path_buf();
		let directory = if path.is_dir() {
			path.clone()
		} else {
			match path.parent() {
				Some(d) => {
					if d == Path::new("") {
						let dir = Path::new(".").to_path_buf();
						if path.is_relative() {
							path = [&dir, &path].iter().collect();
						}
						dir
					} else {
						d.to_path_buf()
					}
				},
				_ => match std::env::current_dir() {
					Ok(i) => i,
					_ => path.clone()
				}
			}
		};
		let images = {
			let mut i = get_image_paths(&directory);
			if path.extension().is_none() && path.is_file() {
				if let Some(e) = path.to_str().map(|e| e.to_string()) {
					i.push(e);
				}
			}
			i
		};
		let index = images.iter().position(|i| Path::new(i) == path).unwrap_or(0);

		Self {
			index,
			images
		}
	}

	fn next(&mut self) -> Option<&str> {
		if self.images.is_empty() {
			return None;
		}
		if self.index == self.images.len() - 1 {
			self.index = 0;
		} else {
			self.index += 1;
		}
		self.get()
	}

	fn prev(&mut self) -> Option<&str> {
		if self.images.is_empty() {
			return None;
		}
		if self.index == 0 {
			self.index = self.images.len() - 1;
		} else {
			self.index -= 1;
		}
		self.get()
	}

	fn get(&self) -> Option<&str> {
		self.images.get(self.index).map(|i| i.as_str())
	}

	fn position(&self) -> usize {
		self.index
	}
	
	fn size(&self) -> usize {
		self.images.len()
	}
}

#[derive(Clone, Event)]
struct ImageViewer {
	widget: rouge::WidgetObject,
	events: rouge::event::Events<Self>,
	zoom: i8,
	scale: f64,
	gallery: Gallery
}

impl ImageViewer {
	fn new(id: &str, gallery: Gallery) -> Self {
		let image = if let Some(path) = gallery.get() {
			rouge::Image::from_path(path)
		} else {
			rouge::Image::new()
		};

		Self {
			widget: ScrollRegionBuilder::new()
				.align(Align::middle())
				.auto_resize(true)
				.display_scrollbars(true)
				.enable_panning(true)
				.scroll_on_arrow_keys(true)
				.scroll_on_mouse_wheel(false)
				.grab_on_mouse_middle(true)
				.build()
				.with_id(id)
				.widget(image)
				.with_on_key_map_press(KeyMap::KeyH, |s, e| {
					if e.only_ctrl() {
						s.toggle_scrollbars();
					}
				})
				.with_on_key_map_press(KeyMap::Home, |s, _| s.go_to_top())
				.with_on_key_map_press(KeyMap::End, |s, _| s.go_to_bottom())
				.into(),
			events: rouge::event::Events::new(),
			zoom: 0,
			scale: Self::SCALE_DEFAULT,
			gallery
		}
			.with_on_scroll_up(|i, e| {
				if i.unwrap_mut().is_region_position(e.position()) {
					i.zoom_in();
				}
			})
			.with_on_scroll_down(|i, e| {
				if i.unwrap_mut().is_region_position(e.position()) {
					i.zoom_out();
				}
			})
			.with_on_key_press(|i, e| {
				if let Some(key) = e.keymap() {
					match key {
						KeyMap::Digit0 | KeyMap::Numpad0 => i.reset(),
						KeyMap::Equal | KeyMap::NumpadAdd => i.zoom_in(),
						KeyMap::Minus | KeyMap::NumpadSubtract => i.zoom_out(),
						/*KeyMap::ArrowLeft | KeyMap::Numpad4 => i.prev(None),
						KeyMap::ArrowRight | KeyMap::Numpad6 => i.next(None),*/
						KeyMap::ArrowUp | KeyMap::Numpad8 => {},
						KeyMap::ArrowDown | KeyMap::Numpad2 => {},
						_ => ()
					}
				}
			})
	}

	fn image(&self) -> &rouge::Image {
		self.unwrap().content_as().unwrap()
	}

	fn image_mut(&mut self) -> &mut rouge::Image {
		self.unwrap_mut().content_as_mut().unwrap()
	}
}

impl rouge::WidgetTime for ImageViewer {}

impl WidgetWrap for ImageViewer {
	fn root(&self) -> &rouge::WidgetObject {
		&self.widget
	}

	fn root_mut(&mut self) -> &mut rouge::WidgetObject {
		&mut self.widget
	}
}

impl WidgetUnwrap for ImageViewer {
	type Wrapped = ScrollRegion;
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum Zoom {
	In(u8),
	Out(u8)
}

impl Zoom {
	fn positioning(self, (x, y): (u32, u32), (w, h): (u32, u32)) -> (i32, i32) {
		match self {
			Self::In(i) => {
				let i = i as u32;
				let (x, y) = ((x / i) + ((w / i) / 2), (y / i) + ((h / i) / 2));
				(x as i32, y as i32)
			},
			Self::Out(i) => {
				let i = i as u32;
				let (wp, hp) = ((w / i) / 2, (h / i) / 2);
				let x = if x > wp { ((x - wp) / (i + 1)) + wp } else { 0 };
				let y = if y > hp { ((y - hp) / (i + 1)) + hp } else { 0 };
				(-(x as i32), -(y as i32))
			}
		}
	}
}

trait Viewer {
	const ZOOM_FACTOR: u8 = 5;

	const ZOOM_IN: Zoom = Zoom::In(Self::ZOOM_FACTOR);

	const ZOOM_OUT: Zoom = Zoom::Out(Self::ZOOM_FACTOR);

	const SCALE_DEFAULT: f64 = 1.0;

	const SCALE_FACTOR: f64 = 1.2;

	const SCALE_MAX: f64 = 0.00390625;

	const SCALE_MIN: f64 = 32.0;

	const TITLE: &'static str = "title";

	fn set(&mut self, _: &Path);

	fn next(&mut self, s: Option<Sender>) {
		let g = self.gallery_mut();
		if g.size() <= 1 {
			return;
		}
		let path = match g.next() {
			Some(p) => p.to_string(),
			_ => return
		};

		self.set(Path::new(&path));
		self.scale();

		if let Some(s) = s {
			if let Some(t) = self.title() {
				s.emit_with(Self::TITLE, t);
			}
		}
	}

	fn prev(&mut self, s: Option<Sender>) {
		let g = self.gallery_mut();
		if g.size() <= 1 {
			return;
		}
		let path = match g.prev() {
			Some(p) => p.to_string(),
			_ => return
		};

		self.set(Path::new(&path));
		self.scale();

		if let Some(s) = s {
			if let Some(t) = self.title() {
				s.emit_with(Self::TITLE, t);
			}
		}
	}

	fn title(&self) -> Option<String> {
		let g = self.gallery();
		let (path, pos, count) = match (g.get(), g.position(), g.size()) {
			(_, _, 0) => return None,
			(Some(a), b, c) => (a.to_string(), b + 1, c),
			_ => return None
		};
		let path = Path::new(&path);
		if let Some(Some(name)) = path.file_name().map(|p| p.to_str()) {
			Some(match self.source_dimensions() {
				(0, 0) => format!("{} - {}/{} - Rim", name, pos, count),
				(w, h) => format!("{} - {}/{} - ({} x {}) - Rim", name, pos, count, w, h)
			})
		} else {
			None
		}
	}

	fn zoom_in(&mut self) {
		if !self.is_valid() {
			return;
		}
		if self.scale_factor() > Self::SCALE_MAX {
			let (x, y) = Self::ZOOM_IN.positioning(self.position(), self.viewport_dimensions());
			{
				let scale = self.scale_factor_mut();
				*scale /= Self::SCALE_FACTOR;
				let sf = *scale;
				let (w, h) = self.source_dimensions();
				let xsf = w as f64 / sf;
				let ysf = h as f64 / sf;
				self.resize((xsf.round() as u32, ysf.round() as u32));
			}
			self.move_by(x, y);
			*self.zoom_level_mut() += 1;
		}
	}

	fn zoom_out(&mut self) {
		if !self.is_valid() {
			return;
		}
		if self.scale_factor() < Self::SCALE_MIN {
			let (x, y) = Self::ZOOM_OUT.positioning(self.position(), self.viewport_dimensions());
			{
				let scale = self.scale_factor_mut();
				*scale *= Self::SCALE_FACTOR;
				let sf = *scale;
				let (w, h) = self.source_dimensions();
				let xsf = w as f64 / sf;
				let ysf = h as f64 / sf;
				self.resize((xsf.round() as u32, ysf.round() as u32));
			}
			self.move_by(x, y);
			*self.zoom_level_mut() -= 1;
		}
	}

	fn reset(&mut self) {
		if !self.is_valid() {
			return;
		}
		let dim = self.dimensions();
		let src = self.source_dimensions();
		if dim != src {
			let zoom = if dim > src { Self::zoom_out } else { Self::zoom_in };
			while self.zoom_level() != 0 {
				zoom(self);
			}
		}
	}

	fn scale(&mut self) {
		if !self.is_valid() {
			return;
		}
		let scale = self.scale_factor();
		let (w, h) = self.source_dimensions();
		let xsf = w as f64 / scale;
		let ysf = h as f64 / scale;
		self.resize((xsf.round() as u32, ysf.round() as u32));
	}

	fn resize(&mut self, _: (u32, u32));

	fn move_by(&mut self, x: i32, y: i32);

	fn dimensions(&self) -> (u32, u32);

	fn source_dimensions(&self) -> (u32, u32);

	fn position(&self) -> (u32, u32);

	fn viewport_dimensions(&self) -> (u32, u32);

	fn zoom_level(&self) -> i8;

	fn zoom_level_mut(&mut self) -> &mut i8;

	fn scale_factor(&self) -> f64;

	fn scale_factor_mut(&mut self) -> &mut f64;

	fn gallery(&self) -> &Gallery;

	fn gallery_mut(&mut self) -> &mut Gallery;

	fn is_valid(&self) -> bool;
}

impl Viewer for ImageViewer {
	fn set(&mut self, path: &Path) {
		self.image_mut().set_with_path(path);
	}

	fn resize(&mut self, size: (u32, u32)) {
		let i = self.image_mut();
		if !i.is_valid() {
			return;
		}
		i.set_size(size);
		i.update();
		self.update();
	}

	fn move_by(&mut self, x: i32, y: i32) {
		self.unwrap_mut().move_by(x, y);
	}

	#[inline(always)]
	fn dimensions(&self) -> (u32, u32) {
		rouge::Widget::dimensions(self.image())
	}

	#[inline(always)]
	fn source_dimensions(&self) -> (u32, u32) {
		self.image().source_dimensions()
	}

	#[inline]
	fn position(&self) -> (u32, u32) {
		self.unwrap().position()
	}

	#[inline]
	fn viewport_dimensions(&self) -> (u32, u32) {
		self.unwrap().region().dimensions()
	}

	#[inline]
	fn zoom_level(&self) -> i8 {
		self.zoom
	}

	fn zoom_level_mut(&mut self) -> &mut i8 {
		&mut self.zoom
	}

	#[inline(always)]
	fn scale_factor(&self) -> f64 {
		self.scale
	}

	#[inline(always)]
	fn scale_factor_mut(&mut self) -> &mut f64 {
		&mut self.scale
	}

	#[inline(always)]
	fn gallery(&self) -> &Gallery {
		&self.gallery
	}

	#[inline(always)]
	fn gallery_mut(&mut self) -> &mut Gallery {
		&mut self.gallery
	}

	#[inline(always)]
	fn is_valid(&self) -> bool {
		self.image().is_valid()
	}
}

const VIEWER: &str = "viewer";
const TOOLBAR: &str = "toolbar";
const NEXT: &str = "next";
const PREV: &str = "prev";
const IN: &str = "in";
const OUT: &str = "out";
const DEFAULT: &str = "default";

#[allow(unused_must_use)]
fn main() {
	let args = std::env::args();
	let path = args.skip(1).next().unwrap_or("".into());

	let viewer = ImageViewer::new(VIEWER, Gallery::new(path));

	let title = viewer.title().unwrap_or_else(|| "Rim".into());
	let size = match Viewer::dimensions(&viewer) {
		(0, 0) => (320, 240),
		(w, h) => (w.clamp(32, 1280), h.clamp(32, 720))
	};

	let icon = |path, sig| {
		rouge::Image::from_path(path)
			.with_padding(4)
			.with_background_color(Color::Rgb(0x34, 0x34, 0x34))
			.with_on_left_click(move |b, _| b.emit(sig))
	};

	let toolbar = rouge::List::new()
		.vertical()
		.space(Space::vertical())
		.with_id(TOOLBAR)
		.with_background_color(Color::Rgb(0x34, 0x34, 0x34))
		.with_relay_bind(true)
		.widget(icon(concat!(env!("CARGO_MANIFEST_DIR"), "/icon/prev.png"), PREV))
		.widget(icon(concat!(env!("CARGO_MANIFEST_DIR"), "/icon/next.png"), NEXT))
		.widget(icon(concat!(env!("CARGO_MANIFEST_DIR"), "/icon/in.png"), IN))
		.widget(icon(concat!(env!("CARGO_MANIFEST_DIR"), "/icon/out.png"), OUT))
		.widget(icon(concat!(env!("CARGO_MANIFEST_DIR"), "/icon/default.png"), DEFAULT));

	let mut fs = false;

	fn find<'a>(w: &'a mut rouge::Window) -> &'a mut ImageViewer {
		w.find_in_all_as_mut::<ImageViewer>(VIEWER).unwrap()
	}

	let prev = |w: &mut rouge::Window| {
		let s = w.channel();
		find(w).prev(Some(s));
		w.update();
	};

	let next = |w: &mut rouge::Window| {
		let s = w.channel();
		find(w).next(Some(s));
		w.update();
	};

	rouge::main(async {
		rouge::App::new()
			.window(rouge::Window::new()
				.title(title)
				.resize(size)
				.visible(true)
				.align(Align::top_left())
				.with_background_color(Color::Rgb(0x24, 0x25, 0x26))
				.with_bind(NEXT, move |w| next(w))
				.with_bind(PREV, move |w| prev(w))
				.with_bind(IN, |w| {
					find(w).zoom_in();
					w.update();
				})
				.with_bind(OUT, |w| {
					find(w).zoom_out();
					w.update();
				})
				.with_bind(DEFAULT, |w| {
					find(w).reset();
					w.update();
				})
				.with_bind_data::<String, _>(ImageViewer::TITLE, |w, t| w.set_title(t))
				.with_on_key_press(move |w, e| {
					if let Some(key) = e.keymap() {
						match key {
							KeyMap::KeyT => {
								if e.only_ctrl() {
									w.find_mut_in_all(TOOLBAR)
										.unwrap()
										.toggle_mapped();
									w.update();
								}
							},
							KeyMap::ArrowLeft | KeyMap::Numpad4 => {
								if e.only_shift() {
									prev(w);
								}
							},
							KeyMap::ArrowRight | KeyMap::Numpad6 => {
								if e.only_shift() {
									next(w);
								}
							},
							KeyMap::PageUp => prev(w),
							KeyMap::PageDown => next(w),
							KeyMap::F11 => {
								w.set_fullscreen({
									fs = !fs;
									fs
								});
							}
							_ => ()
						}
					}
				})
				.widget(rouge::List::new()
					.horizontal()
					.space(Space::both())
					.with_relay_bind(true)
					.widget(toolbar)
					.widget(viewer)))
	});
}