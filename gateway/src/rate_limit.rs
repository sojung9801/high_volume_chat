use tokio::time::Instant;

/// 연결별 토큰 버킷. fan-out 전에 유입을 막아서 방 전체 비용을 줄인다.
pub struct TokenBucket {
    capacity: f64,
    tokens: f64,
    refill_per_sec: f64,
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(burst: u32, refill_per_sec: f64) -> Self {
        let capacity = f64::from(burst);
        Self {
            capacity,
            tokens: capacity,
            refill_per_sec,
            last_refill: Instant::now(),
        }
    }

    pub fn try_acquire(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;
        self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.capacity);

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::TokenBucket;

    // 가상 시간으로 돌린다. 실제로 몇 초씩 기다리지 않고 보충 타이밍을 정확히 본다.

    #[tokio::test(start_paused = true)]
    async fn burst_passes_then_blocks() {
        let mut bucket = TokenBucket::new(5, 1.0);

        let passed = (0..20).filter(|_| bucket.try_acquire()).count();

        assert_eq!(passed, 5);
    }

    #[tokio::test(start_paused = true)]
    async fn refills_one_token_per_interval() {
        let mut bucket = TokenBucket::new(5, 1.0);
        while bucket.try_acquire() {}

        tokio::time::advance(Duration::from_millis(900)).await;
        assert!(!bucket.try_acquire(), "0.9초에는 아직 토큰이 없어야 한다");

        tokio::time::advance(Duration::from_millis(100)).await;
        assert!(bucket.try_acquire(), "1초가 차면 1개가 보충된다");
        assert!(!bucket.try_acquire(), "보충은 1개뿐이다");
    }

    #[tokio::test(start_paused = true)]
    async fn refill_never_exceeds_burst() {
        let mut bucket = TokenBucket::new(5, 1.0);
        while bucket.try_acquire() {}

        tokio::time::advance(Duration::from_secs(3600)).await;
        let passed = (0..20).filter(|_| bucket.try_acquire()).count();

        assert_eq!(passed, 5, "오래 쉬어도 burst를 넘겨 쌓이지 않는다");
    }
}
