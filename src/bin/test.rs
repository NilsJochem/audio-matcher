use std::cell::RefCell;
use std::rc::Rc;

use std::time::Duration;

use audio_matcher::progress_bar::*;
fn main() {
    let bar = Progress::new_bound(
        Bar::<2>::new("test: ".to_owned(), true, Box::new(SimpleArrow::default())),
        0..100,
        4,
    )
    .fit_bound()
    .expect("no resolution found");

    let handles = Rc::new(RefCell::new(Vec::new()));
    let h_copy = Rc::clone(&handles);

    bar.iter_with_finish(
        move |finished, i| {
            std::thread::sleep(Duration::from_millis(100));
            h_copy.borrow_mut().push(std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(500));

                print!(" e{}", i);
                finished();
            }));
        },
    );
    let handles = Rc::try_unwrap(handles).unwrap().into_inner();
    for it in handles {
        it.join().unwrap();
    }
    println!("finished");
}
