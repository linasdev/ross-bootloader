#![no_std]
#![no_main]

use panic_semihosting as _;

use cortex_m::asm::{bootload, nop};
use cortex_m_rt::entry;
use embedded_hal::digital::v2::InputPin;
use stm32f1xx_hal::prelude::*;
use stm32f1xx_hal::pac::Peripherals;
use stm32f1xx_hal::can::Can;
use nb::block;

static PROGRAM_ADDRESS: u32 = 0x0800_8000;
static CAN_BITRATE: u32 = 250_000;
static CAN_TSEG1: u32 = 13;
static CAN_TSEG2: u32 = 2;
static CAN_SJW: u32 = 1;

#[entry]
fn main() -> ! {
    let dp = Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();

    let clocks = rcc
        .cfgr
        .use_hse(8.mhz())
        .sysclk(72.mhz())
        .hclk(72.mhz())
        .pclk1(36.mhz())
        .pclk2(72.mhz())
        .freeze(&mut flash.acr);

    let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);

    let upgrade_input = gpioa.pa1.into_pull_down_input(&mut gpioa.crl);

    // If no firmware upgrade is requested, proceed with bootloading the program
    if upgrade_input.is_low().unwrap() {
        boot();
    }

    let mut afio = dp.AFIO.constrain(&mut rcc.apb2);

    let mut can1 = {
        let can = Can::new(dp.CAN1, &mut rcc.apb1, dp.USB);

        let rx = gpioa.pa11.into_floating_input(&mut gpioa.crh);
        let tx = gpioa.pa12.into_alternate_push_pull(&mut gpioa.crh);
        can.assign_pins((tx, rx), &mut afio.mapr);

        bxcan::Can::new(can)
    };

    can1.configure(|c| {
        c.set_bit_timing(calc_can_btr(clocks.pclk1().0));
        c.set_loopback(false);
        c.set_silent(false);
    });

    block!(can1.enable()).unwrap();

    loop {
        nop();
    }
}

fn boot() -> ! {
    unsafe {
        bootload(PROGRAM_ADDRESS as *const u32);
    }
}

fn calc_can_btr(clock_rate: u32) -> u32 {
    let brp = clock_rate / CAN_BITRATE / (CAN_TSEG1 + CAN_TSEG2);

    (brp - 1) | ((CAN_TSEG1 - 1) << 16) | ((CAN_TSEG2 - 1) << 20) | ((CAN_SJW - 1) << 24)
}
