use std::env;
use std::path::Path;
use std::io::{stdout, Write};
use pdf_extract;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{self, ClearType},
    cursor,
    style::Stylize
};
use textwrap::fill;

struct PdfViewer {
    full_text: String,
    pages: Vec<String>,
    current_page: usize,
    total_pages: usize,
    terminal_width: u16,
    terminal_height: u16,
    pdf_name: String,
}

impl PdfViewer {
    fn new(pdf_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(pdf_path)?;
        let full_text = pdf_extract::extract_text_from_mem(&bytes)
            .map_err(|e| format!("Error al extraer texto del PDF: {}", e))?;
        
        let (terminal_width, terminal_height) = terminal::size()?;
        let pdf_name = Path::new(pdf_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        
        let pages = Self::split_into_pages(&full_text, terminal_width, terminal_height);
        let total_pages = pages.len();
        
        Ok(PdfViewer {
            full_text,
            pages,
            current_page: 0,
            total_pages: if total_pages == 0 { 1 } else { total_pages },
            terminal_width,
            terminal_height,
            pdf_name,
        })
    }

    fn split_into_pages(text: &str, width: u16, height: u16) -> Vec<String> {
        let content_width = (width as usize).saturating_sub(6);
        let content_height = (height as usize).saturating_sub(8);
        
        if text.trim().is_empty() {
            return vec![
                "El PDF parece estar vacío o el texto no se pudo extraer.\n\nEsto puede suceder con:\n• PDFs que son principalmente imágenes\n• PDFs con texto incrustado\n• PDFs con codificación especial\n\nIntenta con un PDF que contenga texto seleccionable.".to_string()
            ];
        }

        let mut pages = Vec::new();
        let mut current_page = String::new();
        let mut lines_in_page = 0;
        
        let page_sections: Vec<&str> = text.split('\x0C').collect(); 
        
        for section in &page_sections {
            let wrapped_text = fill(section, content_width);
            let lines: Vec<&str> = wrapped_text.lines().collect();
            
            for line in lines {
                if lines_in_page >= content_height {
                    pages.push(current_page.trim().to_string());
                    current_page = String::new();
                    lines_in_page = 0;
                }
                
                current_page.push_str(line);
                current_page.push('\n');
                lines_in_page += 1;
            }
            
            if page_sections.len() > 1 {
                if !current_page.trim().is_empty() {
                    pages.push(current_page.trim().to_string());
                    current_page = String::new();
                    lines_in_page = 0;
                }
            }
        }
        
        if !current_page.trim().is_empty() {
            pages.push(current_page.trim().to_string());
        }
        
        if pages.is_empty() {
            let wrapped_text = fill(text, content_width);
            let lines: Vec<&str> = wrapped_text.lines().collect();
            
            let mut page_content = String::new();
            let mut lines_count = 0;
            
            for line in lines {
                if lines_count >= content_height {
                    pages.push(page_content.trim().to_string());
                    page_content = String::new();
                    lines_count = 0;
                }
                page_content.push_str(line);
                page_content.push('\n');
                lines_count += 1;
            }
            
            if !page_content.trim().is_empty() {
                pages.push(page_content.trim().to_string());
            }
        }
        
        pages
    }

    fn draw_page(&self) -> Result<(), Box<dyn std::error::Error>> {
        execute!(stdout(), terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        
        let content_width = (self.terminal_width as usize).saturating_sub(6);
        let content_height = (self.terminal_height as usize).saturating_sub(8);
        
        let header = format!(
            "📄 {} - Página {}/{} 📄", 
            self.pdf_name,
            self.current_page + 1, 
            self.total_pages
        );
        
        println!("{}", header.bold().blue());
        println!(); 
        
        println!("┌{}┐", "─".repeat(content_width + 2));
        
        let page_content = if self.current_page < self.pages.len() {
            &self.pages[self.current_page]
        } else {
            ""
        };
        
        let lines: Vec<&str> = page_content.lines().collect();
        let mut displayed_lines = 0;
        
        for line in lines.iter() {
            if displayed_lines >= content_height {
                break;
            }
            
            let padded_line = format!("{:<width$}", line, width = content_width);
            println!("│ {} │", padded_line);
            displayed_lines += 1;
        }
        
        for _ in displayed_lines..content_height {
            println!("│ {:<width$} │", "", width = content_width);
        }
        
        println!("└{}┘", "─".repeat(content_width + 2));
        println!(); 
        
        let controls = if self.total_pages > 1 {
            "⌨️  Controles: ← Anterior | → Siguiente | q/ESC Salir | r Refrescar"
        } else {
            "⌨️  Controles: q/ESC Salir | r Refrescar"
        };
        
        println!("{}", controls.italic().dark_grey());
        
        if self.total_pages > 1 {
            let progress = format!(
                "Progreso: [{}{}] {:.1}%",
                "█".repeat((self.current_page + 1) * 20 / self.total_pages),
                "░".repeat(20 - (self.current_page + 1) * 20 / self.total_pages),
                ((self.current_page + 1) as f32 / self.total_pages as f32) * 100.0
            );
            println!("{}", progress.dark_cyan());
        }
        
        stdout().flush()?;
        Ok(())
    }

    fn next_page(&mut self) {
        if self.current_page + 1 < self.total_pages {
            self.current_page += 1;
        }
    }

    fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
        }
    }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        terminal::enable_raw_mode()?;
        self.draw_page()?;
        loop {
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key_event) = event::read()? {
                    if key_event.kind == KeyEventKind::Press {
                        match key_event.code {
                            KeyCode::Left | KeyCode::Char('h') => {
                                self.prev_page();
                                self.draw_page()?;
                            }
                            KeyCode::Right | KeyCode::Char('l') => {
                                self.next_page();
                                self.draw_page()?;
                            }
                            KeyCode::Home | KeyCode::Char('g') => {
                                self.current_page = 0;
                                self.draw_page()?;
                            }
                            KeyCode::End | KeyCode::Char('G') => {
                                self.current_page = self.total_pages.saturating_sub(1);
                                self.draw_page()?;
                            }
                            KeyCode::Char('q') | KeyCode::Esc => {
                                break;
                            }
                            KeyCode::Char('r') => {
                                self.draw_page()?;
                            }
                            KeyCode::Char('?') => {
                                execute!(stdout(), terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;
                                println!("{}", "AYUDA - PDF Viewer".bold().green());
                                println!("\n Controles:");
                                println!("  ← / h    : Página anterior");
                                println!("  → / l    : Página siguiente");
                                println!("  Home / g : Primera página");
                                println!("  End / G  : Última página");
                                println!("  r        : Refrescar");
                                println!("  ?        : Mostrar ayuda");
                                println!("  q / ESC  : Salir");
                                println!("\n Información del PDF:");
                                println!("  Archivo: {}", self.pdf_name);
                                println!("  Páginas: {}", self.total_pages);
                                println!("  Caracteres: {}", self.full_text.len());
                                println!("\n Presiona cualquier tecla para volver...");
                                
                                loop {
                                    if let Event::Key(_) = event::read()? {
                                        break;
                                    }
                                }
                                self.draw_page()?;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        
        terminal::disable_raw_mode()?;
        
        execute!(stdout(), terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("{}", "PDF Viewer TUI".bold().blue());
        println!("  ← → h l  : Cambiar páginas");
        println!("  Home/End : Primera/Última página");
        println!("  q ESC    : Salir");
        println!("  r        : Refrescar");
        println!("  ?        : Ayuda");
        std::process::exit(1);
    }
    let pdf_path = &args[1];

    if !Path::new(pdf_path).exists() {
        std::process::exit(1);
    }

    match PdfViewer::new(pdf_path) {
        Ok(mut viewer) => {
            viewer.run()?;
        }
        Err(e) => {
            eprintln!("❌ Error al cargar PDF: {}", e);
            eprintln!("\n💡 Sugerencias:");
            eprintln!("• Verifica que el archivo sea un PDF válido");
            eprintln!("• Algunos PDFs con imágenes pueden no mostrar texto");
            eprintln!("• Prueba con un PDF que contenga texto seleccionable");
            std::process::exit(1);
        }
    }

    Ok(())
}