
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
		let path = path.as_ref();
		let current = |p: &Path| match std::env::current_dir() {
			Ok(p) => p,
			_ => p.to_path_buf()
		};
		let directory = if path.is_dir() {
			path.to_path_buf()
		} else {
			match path.parent() {
				Some(p) => {
					if p == Path::new("") {
						current(p)
					} else {
						p.to_path_buf()
					}
				},
				_ => current(path)
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
	scale: f64,
	gallery: Gallery
}

impl ImageViewer {
	fn new(gallery: Gallery) -> Self {
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
				.with_id("viewer")
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

trait Viewer {
	const SCALE_DEFAULT: f64 = 1.0;

	const SCALE_FACTOR: f64 = 1.2;

	const SCALE_MAX: f64 = 0.00390625;

	const SCALE_MIN: f64 = 32.0;

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
				s.emit_with("title", t);
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
				s.emit_with("title", t);
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
		let scale = self.scale_factor();
		if *scale > Self::SCALE_MAX {
			*scale /= Self::SCALE_FACTOR;
			let sf = *scale;
			let (w, h) = self.source_dimensions();
			let xsf = w as f64 / sf;
			let ysf = h as f64 / sf;
			self.resize((xsf.round() as u32, ysf.round() as u32));
		}
	}

	fn zoom_out(&mut self) {
		if !self.is_valid() {
			return;
		}
		let scale = self.scale_factor();
		if *scale < Self::SCALE_MIN {
			*scale *= Self::SCALE_FACTOR;
			let sf = *scale;
			let (w, h) = self.source_dimensions();
			let xsf = w as f64 / sf;
			let ysf = h as f64 / sf;
			self.resize((xsf.round() as u32, ysf.round() as u32));
		}
	}

	fn reset(&mut self) {
		if self.dimensions() != self.source_dimensions() {
			*self.scale_factor() = Self::SCALE_DEFAULT;
			self.resize(self.source_dimensions());
		}
	}

	fn scale(&mut self) {
		if !self.is_valid() {
			return;
		}
		let scale = *self.scale_factor();
		let (w, h) = self.source_dimensions();
		let xsf = w as f64 / scale;
		let ysf = h as f64 / scale;
		self.resize((xsf.round() as u32, ysf.round() as u32));
	}

	fn resize(&mut self, _: (u32, u32));

	fn dimensions(&self) -> (u32, u32);

	fn source_dimensions(&self) -> (u32, u32);

	fn scale_factor(&mut self) -> &mut f64;

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

	#[inline(always)]
	fn dimensions(&self) -> (u32, u32) {
		rouge::Widget::dimensions(self.image())
	}

	#[inline(always)]
	fn source_dimensions(&self) -> (u32, u32) {
		self.image().source_dimensions()
	}

	#[inline(always)]
	fn scale_factor(&mut self) -> &mut f64 {
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

#[allow(unused_must_use)]
fn main() {
	let args = std::env::args();
	let path = args.skip(1).next().unwrap_or("".into());

	let viewer = ImageViewer::new(Gallery::new(path));

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
		.horizontal()
		.space(Space::horizontal())
		.with_id("toolbar")
		.with_background_color(Color::Rgb(0x34, 0x34, 0x34))
		.with_relay_bind(true)
		.widget(icon(concat!(env!("CARGO_MANIFEST_DIR"), "/icon/prev.png"), "prev"))
		.widget(icon(concat!(env!("CARGO_MANIFEST_DIR"), "/icon/next.png"), "next"))
		.widget(icon(concat!(env!("CARGO_MANIFEST_DIR"), "/icon/in.png"), "in"))
		.widget(icon(concat!(env!("CARGO_MANIFEST_DIR"), "/icon/out.png"), "out"))
		.widget(icon(concat!(env!("CARGO_MANIFEST_DIR"), "/icon/default.png"), "default"));

	let mut fs = false;

	fn find<'a>(w: &'a mut rouge::Window) -> &'a mut ImageViewer {
		w.find_in_all_as_mut::<ImageViewer>("viewer").unwrap()
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
				.with_bind("next", move |w| next(w))
				.with_bind("prev", move |w| prev(w))
				.with_bind("in", |w| {
					find(w).zoom_in();
					w.update();
				})
				.with_bind("out", |w| {
					find(w).zoom_out();
					w.update();
				})
				.with_bind("default", |w| {
					find(w).reset();
					w.update();
				})
				.with_bind_data::<String, _>("title", |w, title| w.set_title(title))
				.with_on_key_press(move |w, e| {
					if let Some(key) = e.keymap() {
						match key {
							KeyMap::KeyT => {
								if e.only_ctrl() {
									w.find_mut_in_all("toolbar")
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
					.space(Space::both())
					.with_relay_bind(true)
					.widget(toolbar)
					.widget(viewer)))
	});
}