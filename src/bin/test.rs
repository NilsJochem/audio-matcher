use std::rc::Rc;
use threadpool::ThreadPool;

use std::time::Duration;

use audio_matcher::progress_bar::*;
fn main() {
    let bar = Progress::new_bound(
        0..10,
        Bar::<2>::new("Progress: ".to_owned(), true, Box::new(SimpleArrow::default())),
        0,
    )
    .fit_bound()
    .expect("no resolution found");

    let pool = Rc::new(ThreadPool::new(25));
    let pool2 = Rc::clone(&pool);

    {
        let (iter, holder) = bar.get_arc_iter();
        for _ in iter {
            let [f1, f2] = OnceCallback::new(&holder);
            f1.call();
            std::thread::sleep(Duration::from_millis(100));
            // f2.call();
            pool2.execute(move || {
                std::thread::sleep(std::time::Duration::from_millis(500));

                // print!(" e{}", i);
                f2.call();
            });
        }
    };
    pool.join();
    println!("finished");
}
