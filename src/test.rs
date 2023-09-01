fn base_price_from_holders(current_holders: u32) -> f64 {
    if current_holders <= 10 {
        0.1 * current_holders as f64
    } else {
        (current_holders as f64 - 10.0) + 1.0
    }
}

fn dual_phase_pricing(current_holders: u32, current_volume: f64, average_volume: f64, time_since_last_trade: f64) -> f64 {
    let volume_adjustment_factor = 0.01;
    let inactivity_adjustment_factor = 0.005;
    let inactivity_threshold = 24.0; 

    let base_price = base_price_from_holders(current_holders);
    
    let volume_ratio = current_volume / average_volume;
    
    if time_since_last_trade > inactivity_threshold {
        base_price * (1.0 - inactivity_adjustment_factor)
    } else {
        base_price * (1.0 + volume_adjustment_factor * volume_ratio)
    }
}

fn main() {
    let average_volume = 7.0; // Setting it in between for simulation purposes.

    // Pump phase
    let pump_holders = [1, 10, 100, 10_000];
    for &holders in pump_holders.iter() {
        let price = dual_phase_pricing(holders, 10.0, average_volume, 1.0);
        println!("Price during pump with {} holders: {} SOL", holders, price);
    }

    // Dump phase
    let dump_holders = [50, 80, 30, 20];
    for &holders in dump_holders.iter() {
        let price = dual_phase_pricing(holders, 5.0, average_volume, 12.0);
        println!("Price during dump with {} holders: {} SOL", holders, price);
    }
}
