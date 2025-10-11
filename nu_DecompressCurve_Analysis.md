# nu_DecompressCurve_0x3409 函数分析报告

本文档旨在分析 `nu_DecompressCurve_0x3409` 函数（原 IDA 函数 `sub_140412DF0`）如何解码 `0x3409` 动画压缩格式中的二进制数据，并将其重建为动画曲线。

## 1. 总体解压流程概述

`0x3409` 格式的解压是一个多步骤过程：

1.  **低级解码（Low-Level Bit Decoding）**: 一个未命名的 BitReader/BitStream 类从压缩数据块中读取指定位数的量化索引（例如 21 位整数）。
2.  **量化反转（Dequantization）**: 量化索引被线性插值到 `[Min, Max]` 浮点数范围内，转换为浮点数索引 `t`。
3.  **曲线重构（Curve Reconstruction）**: `nu_DecompressCurve_0x3409` 函数是核心，它使用 SIMD 载入这些浮点数索引、Min/Max 范围和 Keyframe 数据，执行高级的**非线性插值**以计算最终的平滑动画值。

## 2. nu_DecompressCurve_0x3409 函数分析 (`0x140412DF0`)

该函数是 SIMD (Single Instruction Multiple Data) 优化的，主要使用 `__m128` 向量类型和 SSE/AVX 内建函数，能够并行处理 Vector3 (X, Y, Z) 或 Quaternion (X, Y, Z, W) 的多个分量。

### 2.1 核心数学依赖

该函数严重依赖两个外部 SIMD 数学函数：

| 函数名称 | 功能描述 | 作用 |
| :--- | :--- | :--- |
| `sub_14013DAD0` | SIMD 向量归一化/投影 | 用于将曲线参数映射或缩放到特定的向量空间。 |
| `sub_14006A5C0` | SIMD 向量乘加运算 | 用于执行贝塞尔/样条插值所需的高效矩阵/混合运算。 |

### 2.2 曲线重构逻辑（非线性插值）

函数的核心在于实现文档中提到的 **参数化曲线压缩** 和 **分段线性插值** 的高级数学。

#### A. 模式分支 (Flags/Context)

函数通过检查一个上下文标志位 `v6` 来选择不同的插值参数：

```c
// Line 215-218 in 0x140412DF0 pseudocode
if ( v5 ) // v5 可能是指 is_rotation_track 或 high_quality_mode
  v43 = (__m128)0xC1C80000; // -25.0f (可能用于旋转或高精度模式)
else
  v43 = (__m128)0xC1A00000; // -20.0f
// 不同的模式也选择不同的指数 (v52 = 1.3f vs 1.2f)
```

这些不同的浮点常量（如 `-25.0`, `-20.0`, `1.3`, `1.2`）是调整非线性曲线形状的关键参数（例如 B-Splines 或 Bezier 曲线的控制点/指数）。

#### B. 曲线插值与平滑

代码执行了复杂的 SIMD 混合、乘法和加法序列（Line 223 - 238），用于将输入的原始浮点索引（已经过反量化）映射到最终的曲线值。

```c
// 距离计算/范围确定 (Line 241-245)
// 典型的向量长度计算，用于确定动画变化的幅度或归一化范围。
v50 = _mm_mul_ps(v49, v49);
// ... sumsq and sqrt ...
v87 = _mm_sqrt_ps(_mm_shuffle_ps(v50, v50, 0)); 

// 关键的权重/指数计算
v53 = v52 * v87.m128_f32[0]; // 距离乘以指数 (1.3f 或 1.2f)

// 最终插值（Line 303-305）
v95 = _mm_or_ps(
        _mm_andnot_ps(v59, _mm_add_ps(v87, _mm_mul_ps(_mm_sub_ps(v60, v87), (Constant)))),
        _mm_and_ps(v59, (Clamping_Max_Constant)));
// 这是一个融合了减法、乘法和加法的表达式，是高效实现多项式曲线（如贝塞尔插值）的典型模式。
```

#### C. 边界钳制（Clamping）

函数通过整数位比较生成掩码 `v59`，并使用 `_mm_or_ps`/`_mm_andnot_ps` 指令序列（Line 303-305）将最终解压值限制在预定的浮点数范围内（`Clamping_Max_Constant`）。

```c
// 确保解压后的值不会超出量化时的 Min/Max 范围，防止因浮点误差导致的瑕疵。
v59 = (__m128)_mm_cmpgt_epi32(si128, (__m128i)xmmword_141AA43D0);
// ...

v94 = _mm_or_ps(_mm_and_ps((__m128)xmmword_141B4F7C0, v59), _mm_andnot_ps(v59, v60));
```

## 3. 二进制数值到纯数字的转换流程（模拟 BitReader）

`nu_DecompressCurve_0x3409` 仅处理浮点数，它不执行底层的二进制解密。该二进制解密过程（即**将多个位读取为带符号整数/量化索引**）发生在调用 `nu_DecompressCurve_0x3409` 之前，由一个隐藏的 **BitReader/BitStream** 对象的方法完成。

### 3.1 参数块解析与上下文获取

在将压缩数据转换为数字之前，必须首先从参数块和文件头中获取转换上下文：

- **Z轴参数块**：`FF FF E3 0B 58 71 00 12` (8 bytes)
  - 根据文档推断，该块包含了 **Min/Max 浮点值的引用地址** 和 **Bits Per Entry** 的信息。
  - **关键值**：`Bits Per Entry` 被确定为 21 位。
  - **Min/Max 范围**：根据文件头（非压缩数据）中的 Keyframes，Z轴的范围是 $[0.795937, 90.673424]$。

### 3.2 低级 BitReader 解码（Step 1：获取量化索引）

BitReader 负责从原始字节流中提取指定位数的整数索引。

- **压缩数据**：`01 80 29 35 A7 0F ...`
- **读取位数**：21 位 (Bits Per Entry)

**模拟读取第一个 21 位索引：**

| 字节偏移 | Hex值 | Binary (LSB to MSB) |
| :--- | :--- | :--- |
| 0 | `01` | `1000 0000` |
| 1 | `80` | `0000 0001` |
| 2 | `29` | `1001 0100` |

假设 Little Endian 和 LSB0 顺序：
第一个 21 位索引 (`Index_1`) $= \text{Bytes } 0-2 \text{ 的前 } 21 \text{ 位}$。

$$Index = 0x0...01 \text{ (from B0)} | 0x80 \text{ (from B1)} | 0x29 \text{ (from B2)}$$
...由于我们无法确定 BitReader 的确切位序和实现，我们根据已知结果值 **反推** 索引：

- **反推理论索引**：Z轴第一帧值 $1.721675$ 对应的理论索引约为 **21600 (0x5460)**。

在 C++ 中，BitReader 执行以下操作：
```cpp
// 伪代码：从 BitStream 中读取下一个 'bit_count' 位的整数
uint32_t quantized_index = BitReader::ReadN_Bits(21); 
```

### 3.3 量化反转（Step 2：将索引转换为浮点数）

获取到整数索引后，它通过线性插值公式转换为归一化后的浮点数 $t$：

- **Scale**：$2^{21} - 1 = 2097151$
- **Min/Max**：$0.795937 / 90.674797$

$$t = \text{quantized\_index} / 2097151$$
$$interpolated\_value = 0.795937 \times (1 - t) + 90.674797 \times t$$

这些浮点数值（或其导出的权重）然后被打包，作为输入（例如 `v72`）传递给 `nu_DecompressCurve_0x3409`。

### 3.4 曲线重构（Step 3：使用 SIMD 进行高级插值）

`nu_DecompressCurve_0x3409` 接收这些浮点数作为输入，执行复杂的 **非线性插值**：

- **目的**：将线性的量化结果转换为更符合 Maya 曲线特征（如 Ease-Out, 三次 Bezier）的平滑动画值。
- **机制**：通过 SIMD 乘加和 `sub_14013DAD0` 等函数执行复杂的向量数学，例如：
  $$v_{\text{final}} = \text{Blend}\left( v_{\text{t}} \times (1.2 \text{ or } 1.3 \text{ scale}) \right)$$
  所有操作都在 SIMD 寄存器中高速完成，确保动画曲线输出的准确性和性能。
