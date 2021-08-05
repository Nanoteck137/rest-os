use spin::Mutex;

use super::{ out8, in8 };

struct SerialPort {
    port: u16,
}

impl SerialPort {
    fn new(port: u16) -> Self {
        unsafe {
            out8(port + 1, 0x00); // Disable all interrupts
            out8(port + 3, 0x80); // Enable DLAB (set baud rate divisor)
            out8(port + 0, 0x03); // Set divisor to 3 (lo byte) 38400 baud
            out8(port + 1, 0x00); //                  (hi byte)
            out8(port + 3, 0x03); // 8 bits, no parity, one stop bit
            out8(port + 2, 0xC7); // Enable FIFO, clear them, with 14-byte threshold
            out8(port + 4, 0x0B); // IRQs enabled, RTS/DSR set
            out8(port + 4, 0x0F);
        }

        Self {
            port
        }
    }

    fn is_transmit_empty(&self) -> bool {
        return unsafe { in8(self.port + 5) } & 0x20 != 0;
    }

    fn output_char(&mut self, c: char) {
        while !self.is_transmit_empty() {}

        unsafe {
            out8(self.port, c as u8);
        }
    }
}

impl core::fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            self.output_char(c);
        }

        Ok(())
    }
}

static SERIAL_PORT: Mutex<Option<SerialPort>> = Mutex::new(None);

pub fn initialize() {
    {
        *SERIAL_PORT.lock() = Some(SerialPort::new(0x3f8));
    }
}

pub fn print_fmt(args: core::fmt::Arguments) {
    use core::fmt::Write;

    let mut lock = SERIAL_PORT.lock();
    match lock.as_mut() {
        Some(f) => {
            f.write_fmt(args).unwrap();
        }
        None => {
        }
    }
}
