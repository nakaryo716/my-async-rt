use my_async_rt::{count_fut::CounterFut, runtime::Runtime};

fn main() {
    let rt = Runtime::new();

    let future = async {
        let task1 = rt.spawn(CounterFut::new(4)).unwrap();

        let task2 = rt.spawn(CounterFut::new(3)).unwrap();

        println!("{}", task1.await);
        println!("{}", task2.await);

        let val = CounterFut::new(2);
        println!("{}", val.await);
    };

    rt.block_on(future);
}
