use std::mem;

const MAX_BUF_LEN: usize = 512;

#[derive(Debug)]
pub struct MessageBuf {
    pub pool_id: u8,
    pub data: Box<[u8]>,
    pub len: usize,
}

type Slot = Option<(MessageBuf, usize)>;

pub struct MessageBufPool {
    pub pool_id: u8,
    slots: Vec<Slot>,
    len: usize,
    next: usize,
}

impl MessageBufPool {
    pub fn new(pool_id: u8, len: usize) -> Self {
        let mut slots = (0..len).fold(Vec::with_capacity(len), |mut v, i| {
            v.push(Some((
                MessageBuf {
                    pool_id: pool_id,
                    data: Box::new([0; MAX_BUF_LEN]),
                    len: 0,
                },
                i + 1,
            )));
            v
        });

        MessageBufPool {
            pool_id,
            slots,
            len,
            next: 0,
        }
    }

    pub fn allocate(&mut self) -> Option<MessageBuf> {
        if self.next == self.len {
            None
        } else {
            let prev = mem::replace(&mut self.slots[self.next], None);
            if let Some((buf, next)) = prev {
                self.next = next;
                return Some(buf);
            }
            panic!("next should point to usable buf");
        }
    }

    pub fn release(&mut self, mut buf: MessageBuf) {
        assert!(buf.pool_id == self.pool_id);

        if let Some(free_index) = self.slots.iter().position(|s| s.is_none()) {
            buf.len = 0;
            self.slots[free_index] = Some((buf, self.next));
            self.next = free_index;
        } else {
            panic!("release buf not allocate by this pool");
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_allocate_and_release() {
        let mut pool = MessageBufPool::new(2);
        let mut buf = pool.allocate().unwrap();
        buf.data[0] = 1;
        pool.release(buf);

        let buf1 = pool.allocate().unwrap();
        assert_eq!(buf1.data[0], 1);
        let mut buf2 = pool.allocate().unwrap();
        buf2.data[0] = 2;
        assert!(pool.allocate().is_none());
        pool.release(buf2);
        let buf = pool.allocate().unwrap();
        assert_eq!(buf.data[0], 2);
    }
}
