## 荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 **以下各位** 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

    > *暂无*

2. 此外，我也参考了 **以下资料** ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

    > rCore-Tutorial-Guide-2025S  文档

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。
4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

### 编程作业

这次实验我主要完成了`sys_get_time`，`sys_mmap`，`sys_munmap`，`sys_spawn`和 stride 调度算法。其中`sys_spawn`的收获是比较大的，因为一开始我参考fork代码进行编写，错误地设置了内核栈，导致其他所有用例都测不了，正确设置栈是非常关键的。stride调度算法主要是找准BigStride值，并且在调度后计算好步长，更新stride。`sys_get_time`，`sys_mmap`，`sys_munmap`这三个函数，改变不大，主要是找好token就可以成功。


### 简答作业

#### stride 算法原理非常简单，但是有一个比较大的问题。例如两个 pass = 10 的进程，使用 8bit 无符号整形储存 stride， p1.stride = 255, p2.stride = 250，在 p2 执行一个时间片后，理论上下一次应该 p1 执行。

##### 实际情况是轮到 p1 执行吗？为什么？

不会，使用 8bit 无符号整形最大值是255, 当 stride = 250，再加上10就溢出了，变成了4，所以p2的stride依然是最小的，它还是被先执行。


#### 我们之前要求进程优先级 >= 2 其实就是为了解决这个问题。可以证明， 在不考虑溢出的情况下 , 在进程优先级全部 >= 2 的情况下，如果严格按照算法执行，那么 STRIDE_MAX – STRIDE_MIN <= BigStride / 2。

##### 为什么？尝试简单说明（不要求严格证明）。

根据算法设定 P.pass = BigStride / P.priority，当优先级 ≥ 2 时，最大步长 max_pass = BigStride / 2，当某个进程 P1 的 stride 成为 STRIDE_MAX 时，必然存在某个进程 P2 的 stride 是 STRIDE_MIN，由于 P2 的 pass 值 ≤ BigStride/2（优先级≥2），当 P2 被调度后：

```
new_stride(P2) = STRIDE_MIN + pass(P2) ≤ STRIDE_MIN + BigStride/2
```

调度器永远选择当前 stride 值最小的进程运行，P2被调度后，只有`new_stride(P2)` > `STRIDE_MAX`，才会让P1再次被调度，因此：

```
STRIDE_MAX < new_stride(P2) = STRIDE_MIN + pass(P2) ≤ STRIDE_MIN + BigStride/2
```

转换之后：

```
STRIDE_MAX - STRIDE_MIN ≤ BigStride/2
```

##### 已知以上结论，考虑溢出的情况下，可以为 Stride 设计特别的比较器，让 BinaryHeap<Stride> 的 pop 方法能返回真正最小的 Stride。补全下列代码中的 partial_cmp 函数，假设两个 Stride 永远不会相等。

(咨询了AI关于wrapping_sub的用法以及思路)


```rust
use core::cmp::Ordering;

struct Stride(u64);

impl PartialOrd for Stride {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        const BIG_STRIDE: u8 = 255;
        const HALF_STRIDE: u8 = BIG_STRIDE / 2; // 127

        // 将 u64 值视为 8 位无符号整数（隐式截断高位）
        let a = self.0 as u8;
        let b = other.0 as u8;

        // 计算从 a 到 b 的顺时针距离
        let distance = b.wrapping_sub(a);

        // 判断是否跨越环形中点
        if distance <= HALF_STRIDE {
            Some(Ordering::Less) // a 在 b 的顺时针半环内，a < b
        } else {
            Some(Ordering::Greater) // a 在 b 的逆时针半环内，a > b
        }
    }
}

impl PartialEq for Stride {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}
```