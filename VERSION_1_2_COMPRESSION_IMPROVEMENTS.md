# Maya .anim 动画数据压缩算法逆向工程文档

## 项目概述

### 目标
设计并逆向工程一个高效的动画数据压缩算法，用于存储Maya导出的`.anim`文件，以最大化节省存储空间。

### 技术栈
- **主要语言**: C
- **数据源**: Maya动画关键帧数据
- **压缩目标**: 将浮点数时间序列压缩为二进制格式

---

## 一、原始数据特征分析

### 1.1 样本数据概览

```yaml
动画信息:
  - 帧数 (Frame count): 40
  - Unk1: 1.0
  - Unk2: 28.874197
  - Unk3: 2
  - Unk4: 21
  
关键帧（已提取，不在压缩数据中）:
  - First (帧1):  (0.000000, 0.000000, 0.795940)
  - Middle (帧34): (0.000000, 0.000000, 71.693703)
  - Last (帧40):  (0.000000, 0.000000, 90.674797)

变化特征:
  - 仅 translateZ 轴有变化
  - translateX, translateY 保持为 0
  - 数值呈非线性增长（ease-out曲线）
```

### 1.2 Maya原始数据格式

```
anim translate.translateZ translateZ GBL_RT 0 1 2;
animData {
  input time;
  output linear;
  weighted 0;
  preInfinity constant;
  postInfinity constant;
  keys {
    1.00002 0.795937 fixed fixed 1 0 0 42.792758 1 42.792758 1;
    1.9999796 1.721675 linear linear 1 0 0;
    3 2.773587 linear linear 1 0 0;
    ...
    40.00002 90.673424 linear linear 1 0 0;
  }
}
```

**数据量统计**:
- 原始数据: 40帧 × 8字节(时间+值) = **320字节**
- 压缩目标: 减少至 **15-60字节** (压缩率 80-95%)

---

## 二、压缩算法理论方案

### 2.1 方案1: 参数化曲线压缩 ⭐ (最推荐)

#### 原理
将离散关键帧拟合为数学函数，仅存储函数参数。

#### 数据结构
```c
typedef struct {
    uint8_t  curve_type;      // 曲线类型 (0=linear, 1=ease-in, 2=ease-out, 3=bezier)
    uint16_t frame_count;     // 总帧数
    float    start_value;     // 起始值
    float    end_value;       // 结束值
    float    control_params[4]; // 控制参数
} CompressedCurve;

// 存储空间: 1 + 2 + 4 + 4 + 16 = 27 bytes
// 压缩率: 91.6%
```

#### 曲线类型枚举
```c
enum CurveType {
    CURVE_LINEAR = 0,
    CURVE_EASE_IN = 1,      // t^2
    CURVE_EASE_OUT = 2,     // 1 - (1-t)^2
    CURVE_EASE_IN_OUT = 3,  // smoothstep
    CURVE_BEZIER = 4,       // 三次贝塞尔
    CURVE_HERMITE = 5       // Hermite样条
};
```

#### 解压函数
```c
float evaluate_curve(CompressedCurve* curve, float frame) {
    float t = frame / (float)curve->frame_count;  // 归一化 [0,1]
    float normalized_value;
    
    switch(curve->curve_type) {
        case CURVE_EASE_OUT:
            normalized_value = 1.0f - powf(1.0f - t, 2.0f);
            break;
        case CURVE_BEZIER:
            normalized_value = cubic_bezier(t, 
                curve->control_params[0], 
                curve->control_params[1], 
                curve->control_params[2], 
                curve->control_params[3]);
            break;
    }
    
    return curve->start_value + normalized_value * (curve->end_value - curve->start_value);
}
```

#### 针对样本数据的优化格式
```c
typedef struct {
    uint8_t  type;          // 2 (EASE_OUT)
    uint16_t frames;        // 40
    float    start;         // 0.795937
    float    end;           // 90.673424
    float    exponent;      // 2.0 (ease-out power)
} OptimalFormat;

// 仅需 1 + 2 + 4 + 4 + 4 = 15 bytes
// 压缩率: 95.3%
```

### 2.2 方案2: 自适应关键帧 + 样条插值

#### 原理
使用 Ramer-Douglas-Peucker 算法筛选关键帧，其余帧通过插值重建。

```c
typedef struct {
    uint16_t frame_indices[MAX_KEYFRAMES];  // 关键帧索引
    float    values[MAX_KEYFRAMES];         // 关键帧值
    uint8_t  keyframe_count;                // 关键帧数量
    uint8_t  interpolation_type;            // 插值类型
} AdaptiveKeyframes;

// 典型结果: 40帧 → 5-8个关键帧
// 存储: 8*(2+4) = 48 bytes
// 压缩率: 85%
```

### 2.3 方案3: 差分 + 可变长编码

```c
typedef struct {
    float    base_value;           // 首帧值
    uint16_t frame_count;
    uint8_t  quantization_bits;    // 量化位数 (如10位)
    float    delta_range;          // 差分值范围
    uint8_t  deltas[];             // 可变长数组（位打包）
} DeltaCompressed;

// 存储: 4 + 2 + 1 + 4 + (40 * 10 / 8) = 61 bytes
// 压缩率: 81%
```

### 2.4 方案4: DCT频域压缩

```c
// 类似JPEG，保留低频分量
void dct_compress(float* signal, int n, int keep_coeffs) {
    float dct_coeffs[MAX_FRAMES];
    discrete_cosine_transform(signal, n, dct_coeffs);
    
    // 只保留前keep_coeffs个系数（通常8-12个）
    for(int i = keep_coeffs; i < n; i++) {
        dct_coeffs[i] = 0;
    }
}
// 压缩率: 70-85%
```

---

## 三、实际二进制数据逆向分析

### 3.1 完整Hex Dump (117字节)

```
01 00 01 00 01 00 08 12  01 00 01 00 01 00 08 12
FF FF E3 0B 58 71 00 12  01 80 29 35 A7 0F 9A 0E
04 60 FD 7F 7A 31 03 43  A6 1B E4 2B B6 0E 01 20
36 18 8B 44 6A 0F 49 34  8B 08 CB 2A 5E 02 43 24
72 FE 36 1E 50 FC 2D 19  0F FA 01 16 4B F7 A6 13
F5 11 F4 0E 01 00 01 00  01 00 02 11 01 00 01 00
01 00 02 11 B5 04 AC 14  FF 11 00 11 06 80 97 F6
C1 2F 6F 0B C7 7F 18 BE
```

### 3.2 数据分段结构

[0-7]:   01 00 01 00 01 00 08 12  // 段头1（可能是X轴）
[8-15]:  01 00 01 00 01 00 08 12  // 段头2（可能是Y轴）
[16-23]: FF FF E3 0B 58 71 00 12  // 参数块1  FF FF E3 0B不知道代表什么，可能是杂凑算法？，或者解法？用来控制24-81区域的大小，固定是被0x4整除的，不确定是否是四组看[FF FF E3 0B] or [FF FF E3 0B 58 71 00]，因为我发现固定是12,12肯定是一个参数 [xx xx xx xx xx xx xx 12]，已知 [....08 12, ....02 11]后面没有压缩数据，下面的另一种[02 11] 后面也没有压缩数据，但[00 11]就有，这是一种pattern，所以，我猜[... 00 12 和 .. 00 11] = 后面有压缩数据
[24-81]: 01 80 29 35 A7 0F...0E   // 数据块1（58字节）← Z轴压缩数据

[82-89]:  01 00 01 00 01 00 02 11 // 段头3 （可能是X轴）
[90-97]:  01 00 01 00 01 00 02 11 // 段头4（可能是Y轴）
[98-105]: B5 04 AC 14 FF 11 00 11 // 参数块2
[106-116]: 06 80 97 F6...18 BE    // 数据块2（11字节）

可见每次压缩的数据都包含了两组xyz

这是第二个anim的压缩文件
1D 37 AC 12 3A 26 00 12 02 80 5E 1D CD 23 48 08 00 1A F9 7F 4C 55 C7 1C 0D 1D 1B 08 0E 61 3E 20 19 36 24 13 0D 26 10 11 0B 13 07 0D 02 09 00 06 FF FF DF 17 67 71 00 12 FF 7F 74 FA 6D CD 82 F3 33 5E 94 B1 05 80 EA FA 29 1B 12 D6 B3 CC AE FF 13 06 85 A8 B8 BC 63 01 71 F4 EF D0 21 E7 FA FE 04 F5 A5 E6 4F F1 04 FA 72 F7 DA F3 FD F8 8D FA FD FD FD FD 23 67 E7 28 5F 71 00 12 02 80 E5 E0 CF 2F 7B 14 9F 00 28 77 FE 7F EC 18 59 13 36 2F DE 16 7A FB 25 F4 74 09 85 0D 13 F3 E3 F5 D4 14 7A 14 92 FE 55 07 C7 12 00 0A 9B 01 F8 06 58 06 B1 03 5F 01 01 FF 01 00 42 0A B2 18 3C 17 00 12 F6 7F 9C F5 4A D8 BE EE 81 D9 BF EE E3 F0 EE F6 C2 E8 D4 EB E2 F1 E7 F7 EB FA F1 FB F4 FF F6 01 F8 02 FA 04 55 37 8B 16 3F 26 00 12 02 80 7C 0D 31 3F FA 06 FC 7F AB 07 DF 2B 9B 05 1F 08 17 05 3E 07 2A 01 23 01 1F 00 19 FC 14 FA 11 FA 10 F9 0E F7 0C F6 EF 2B 83 19 27 17 00 12 FC 7F D5 94 39 F6 B2 0B 81 38 C8 0D ED 02 F5 04 BD 17 D3 09 E3 09 E5 0C E9 09 EF 08 F0 0A F1 0A F4 0A F5 0B

同样
1. 1D 37 AC 12 3A 26 00 12
压缩数据
02 80 5E 1D CD 23 48 08 00 1A F9 7F 4C 55 C7 1C 0D 1D 1B 08 0E 61 3E 20 19 36 24 13 0D 26 10 11 0B 13 07 0D 02 09 00 06

2. FF FF DF 17 67 71 00 12
压缩数据
FF 7F 74 FA 6D CD 82 F3 33 5E 94 B1 05 80 EA FA 29 1B 12 D6 B3 CC AE FF 13 06 85 A8 B8 BC 63 01 71 F4 EF D0 21 E7 FA FE 04 F5 A5 E6 4F F1 04 FA 72 F7 DA F3 FD F8 8D FA FD FD FD FD

3. 23 67 E7 28 5F 71 00 12
压缩数据
02 80 E5 E0 CF 2F 7B 14 9F 00 28 77 FE 7F EC 18 59 13 36 2F DE 16 7A FB 25 F4 74 09 85 0D 13 F3 E3 F5 D4 14 7A 14 92 FE 55 07 C7 12 00 0A 9B 01 F8 06 58 06 B1 03 5F 01 01 FF 01 00

4. 42 0A B2 18 3C 17 00 12
压缩数据
F6 7F 9C F5 4A D8 BE EE 81 D9 BF EE E3 F0 EE F6 C2 E8 D4 EB E2 F1 E7 F7 EB FA F1 FB F4 FF F6 01 F8 02 FA 04 

5. 55 37 8B 16 3F 26 00 12
压缩数据
02 80 7C 0D 31 3F FA 06 FC 7F AB 07 DF 2B 9B 05 1F 08 17 05 3E 07 2A 01 23 01 1F 00 19 FC 14 FA 11 FA 10 F9 0E F7 0C F6

6. EF 2B 83 19 27 17 00 12
压缩数据
FC 7F D5 94 39 F6 B2 0B 81 38 C8 0D ED 02 F5 04 BD 17 D3 09 E3 09 E5 0C E9 09 EF 08 F0 0A F1 0A F4 0A F5 0B

可见压缩数据中也有重复，比如FD FD FD FD，也许这是一种要二进制看的数据？

还有文件3
1. 63 1E 90 07 DC 06 20 12
压缩数据
3D 81 F4 00 45 03 7F 48 6D 40 67 3B 60 34 50 27 3C 19 2F 0F 23 07 1C 01 99 88 99 87

2. FF FF A3 0D 91 17 00 12
压缩数据，我找不到结尾
01 80 38 FD A6 08 B4 07 7F 75 60 59 48 43 35 31 43 3F 30 2F 23 24 1B 1C 15 17 10 12 0C 0E 07 0A 04 06 00 03 E4 07 D2 54 41 04 40 12 3F 85 81 BA 81 D5 00 19 20 20 1E 1B 60 55 48 3F BA BA BA A9 98 98 98 98 75 02 27 27 FF 01 20 11

另外！
我给你的数据全都只是用来控制translate，不涉及rotation和scale

### 3.3 通道头结构定义

```c
typedef struct {
    uint8_t field1;      // [0] = 0x01 (固定?)
    uint8_t field2;      // [1] = 0x00
    uint8_t field3;      // [2] = 0x01
    uint8_t field4;      // [3] = 0x00
    uint8_t field5;      // [4] = 0x01
    uint8_t field6;      // [5] = 0x00
    uint8_t data_flag;   // [6] ← 关键标志位
    uint8_t channel_id;  // [7] ← 通道类型/索引
} ChannelHeader;  // 总计 8 字节
```

### 3.4 数据标志位规律

| data_flag | channel_id | 后续数据 | 含义 |
|-----------|------------|---------|------|
| `0x08`    | `0x12`     | ❌ 无    | 常量轴（无变化）|
| `0x02`    | `0x11`     | ❌ 无    | 常量轴（另一类型）|
| `0x00`    | `0x12`     | ✅ 有    | 包含压缩数据 |
| `0x00`    | `0x11`     | ✅ 有    | 包含压缩数据 |

```c
#define FLAG_NO_DATA      0x08  // 轴无变化
#define FLAG_CONSTANT     0x02  // 可能是另一种常量表示
#define FLAG_COMPRESSED   0x00  // 后面跟随压缩数据块
```

### 3.5 参数块分析

#### Z轴参数块: `FF FF E3 0B 58 71 00 12`

```c
// 可能的解析方案A: 4字节分组
struct ParamBlockA {
    uint32_t param1;  // 0x0BE3FFFF
    uint32_t param2;  // 0x12007158
};

// 可能的解析方案B: 混合类型
struct ParamBlockB {
    int16_t  marker;        // 0xFFFF = -1 (标记位?)
    uint16_t scale_factor;  // 0x0BE3 = 3043 (量化缩放?)
    uint16_t unknown1;      // 0x7158 = 29016
    uint8_t  data_flag;     // 0x00
    uint8_t  channel_id;    // 0x12
};

// 可能的解析方案C: 与unk参数对应
// unk2 = 28.874197 的IEEE 754编码:
//   大端序: 41 E6 FD 70
//   小端序: 70 FD E6 41
// 但数据中是: E3 0B 58 71 (不直接匹配)
```

#### 缩放因子假设

```c
uint16_t scale = 0x0BE3; // = 3043 (十进制)

// 可能用途:
// 1. 定点数量化的缩放因子
// 2. 差分值的归一化范围
// 3. 与帧数/时间相关的计算参数
```

### 3.6 压缩数据块模式

#### 数据头部观察: `01 80, 06 80, FF 7F, 02 80, F6 7F, FC 7F...`

```c
// 按16位有符号整数解析（小端序）:
0x8001 = -32767
0x8006 = -32762  
0x7FFF = +32767
0x8002 = -32766
0x7FF6 = +32758
0x7FFC = +32764

// 特征: 都接近 ±32768 = ±2^15
// 推测: 使用16位整数存储量化后的增量
```

#### 编码假设

```c
// 假设1: 定点数增量编码
typedef struct {
    int16_t control_word;  // 如 0x8001
    int16_t deltas[];      // 后续增量值（16位定点数）
} FixedPointBlock;

// 假设2: 变长整数 (Varint)
typedef struct {
    uint8_t encoding_type;  // 0x01
    uint8_t flags;          // 0x80 (MSB设置，表示多字节)
    uint8_t varints[];      // 变长编码的增量
} VarintBlock;
```

---

## 四、解码算法验证框架

### 4.1 测试数据准备

```c
// 已知真实值（去除first/middle/last后的37帧）
float known_values[] = {
    1.721675,  2.773587,  3.944817,  5.231249,  6.627399,
    8.129151,  9.729649,  11.42615,  13.21042,  15.0811,
    17.029949, 19.05286,  21.144341, 23.30028,  25.51519,
    27.784969, 30.10137,  32.46302,  34.863091, 37.294689,
    39.75647,  42.240189, 44.743111, 47.258369, 49.780499,
    52.35062,  54.963249, 57.621151, 60.328419, 63.08506,
    65.896561, 68.765663,  // 第33帧
    // 跳过第34帧 (middle)
    74.683533, 77.739143, 80.863342, 84.058853, 87.328407
    // 第40帧 (last) 不包含
};

// Z轴压缩数据（58字节）
uint8_t z_compressed[] = {
    0x01, 0x80, 0x29, 0x35, 0xA7, 0x0F, 0x9A, 0x0E,
    0x04, 0x60, 0xFD, 0x7F, 0x7A, 0x31, 0x03, 0x43,
    0xA6, 0x1B, 0xE4, 0x2B, 0xB6, 0x0E, 0x01, 0x20,
    0x36, 0x18, 0x8B, 0x44, 0x6A, 0x0F, 0x49, 0x34,
    0x8B, 0x08, 0xCB, 0x2A, 0x5E, 0x02, 0x43, 0x24,
    0x72, 0xFE, 0x36, 0x1E, 0x50, 0xFC, 0x2D, 0x19,
    0x0F, 0xFA, 0x01, 0x16, 0x4B, 0xF7, 0xA6, 0x13,
    0xF5, 0x11, 0xF4, 0x0E
};
```

### 4.2 验证程序1: 定点数解码

```c
#include <stdio.h>
#include <stdint.h>
#include <math.h>

void test_fixed_point_decode(uint8_t* data, int len) {
    printf("\n=== 16位定点数解码测试 ===\n");
    
    float scale_factors[] = {100.0f, 1000.0f, 3043.0f, 10000.0f, 32768.0f};
    
    for(int s = 0; s < 5; s++) {
        printf("\n--- Scale Factor: %.0f ---\n", scale_factors[s]);
        
        float current = 0.795937;  // first frame
        int offset = 2;  // 跳过 01 80
        
        for(int i = 0; i < 37 && offset + 1 < len; i++) {
            // 小端序读取16位整数
            int16_t delta_raw = (int16_t)(data[offset] | (data[offset+1] << 8));
            offset += 2;
            
            float delta = (float)delta_raw / scale_factors[s];
            current += delta;
            
            float error = fabsf(current - known_values[i]);
            
            printf("Frame %2d: raw=%6d, delta=%8.3f, value=%8.3f, "
                   "real=%8.3f, error=%8.6f\n",
                   i+2, delta_raw, delta, current, known_values[i], error);
            
            // 如果误差过大，停止此次测试
            if(error > 1.0f) break;
        }
    }
}

int main() {
    test_fixed_point_decode(z_compressed, sizeof(z_compressed));
    return 0;
}
```

### 4.3 验证程序2: Varint解码

```c
int decode_varint_zigzag(uint8_t* data, int* offset, int32_t* out_value) {
    uint32_t result = 0;
    int shift = 0;
    
    while(1) {
        uint8_t byte = data[(*offset)++];
        result |= (byte & 0x7F) << shift;
        
        if((byte & 0x80) == 0) break;  // MSB=0，结束
        shift += 7;
    }
    
    // ZigZag解码: 0→0, 1→-1, 2→1, 3→-2...
    *out_value = (result >> 1) ^ -(result & 1);
    return 1;
}

void test_varint_decode(uint8_t* data, int len) {
    printf("\n=== Varint + ZigZag 解码测试 ===\n");
    
    int offset = 2;  // 跳过 01 80
    float current = 0.795937;
    float scale = 1000.0f;
    
    for(int i = 0; i < 37 && offset < len; i++) {
        int32_t signed_val;
        decode_varint_zigzag(data, &offset, &signed_val);
        
        float delta = signed_val / scale;
        current += delta;
        
        printf("Frame %2d: signed=%6d, value=%.6f (real=%.6f, error=%.6f)\n",
               i+2, signed_val, current, known_values[i], 
               fabsf(current - known_values[i]));
    }
}
```

### 4.4 验证程序3: 参数查找

```c
void find_unk_parameters(uint8_t* param_block) {
    printf("\n=== 参数块解析 ===\n");
    
    // 测试所有可能的32位float位置
    for(int i = 0; i <= 4; i++) {
        float val = *(float*)(&param_block[i]);
        printf("Offset %d as float: %f\n", i, val);
    }
    
    // 测试16位整数
    for(int i = 0; i < 8; i += 2) {
        uint16_t u16 = *(uint16_t*)(&param_block[i]);
        int16_t s16 = *(int16_t*)(&param_block[i]);
        printf("Offset %d as uint16: %u, int16: %d\n", i, u16, s16);
    }
    
    // 检查是否包含 unk2 = 28.874197
    float unk2 = 28.874197f;
    uint32_t unk2_bits = *(uint32_t*)(&unk2);
    printf("\nunk2 IEEE 754 bits: 0x%08X\n", unk2_bits);
}
```

---

## 五、当前结论与假设

### 5.1 确定性发现

✅ **数据组织结构**:
- 每个通道（X/Y/Z）使用8字节头部
- 无变化的轴通过标志位标记（0x08或0x02）
- 有变化的轴后跟参数块+压缩数据

✅ **压缩特征**:
- Z轴 37帧压缩为 58字节（约1.57字节/帧）
- 数据采用某种定点数或变长整数编码
- 可能使用差分编码减少数据量

✅ **三关键帧策略**:
- First/Middle/Last帧从压缩数据中排除
- 可能用于辅助解码或误差校验

### 5.2 待验证假设

❓ **缩放因子**:
- `0x0BE3 (3043)` 可能是定点数的缩放因子
- 需要验证: `delta_real = delta_raw / 3043`

❓ **控制字含义**:
- `01 80` 可能是编码类型标识
- 或者是数据块的长度/范围标记

❓ **Unk参数映射**:
- unk1-4 在二进制中的确切位置未知
- unk2 (28.874197) 未在参数块中找到匹配

---

## 六、下一步行动计划

### 6.1 立即验证任务

1. **运行验证程序**
   ```bash
   gcc -o test_decode test_decode.c -lm
   ./test_decode
   ```

2. **收集更多样本**
   - 至少2-3个不同的.anim文件
   - 包含完整头部（帧数、unk参数）
   - 已知所有关键帧的数值

3. **定位Unk参数**
   - 在完整二进制文件中查找 `40` (帧数)
   - 查找 `28.874197` 的IEEE 754编码 (0x41E6FD70)
   - 确认unk3=2, unk4=21的存储位置

### 6.2 长期研究方向

- [ ] 实现完整的编码/解码库
- [ ] 测试多种曲线类型的拟合效果
- [ ] 开发自动化格式探测工具
- [ ] 编写Maya插件实现实时压缩

---

## 七、参考资料

### 代码示例完整版

详见附件:
- `fixed_point_decode.c` - 定点数解码测试
- `varint_decode.c` - 变长整数解码测试
- `param_finder.c` - 参数查找工具

### 相关技术文档

- IEEE 754浮点数标准
- Protocol Buffers Varint编码
- Ramer-Douglas-Peucker算法
- DCT (Discrete Cosine Transform)
- 缓动函数数学原理

---

## 附录A: 误差计算函数

```c
// 计算均方根误差 (RMSE)
float calculate_rmse(float* original, float* decoded, int count) {
    float sum_sq_error = 0;
    for(int i = 0; i < count; i++) {
        float diff = original[i] - decoded[i];
        sum_sq_error += diff * diff;
    }
    return sqrtf(sum_sq_error / count);
}

// 计算最大绝对误差
float max_absolute_error(float* original, float* decoded, int count) {
    float max_err = 0;
    for(int i = 0; i < count; i++) {
        float err = fabsf(original[i] - decoded[i]);
        if(err > max_err) max_err = err;
    }
    return max_err;
}

// 计算相对误差百分比
float relative_error_percent(float original, float decoded) {
    if(fabsf(original) < 1e-6f) return 0;
    return fabsf((decoded - original) / original) * 100.0f;
}
```

---

## 附录B: 数据可视化建议

```python
# Python脚本: 绘制原始vs解码曲线
import matplotlib.pyplot as plt
import numpy as np

frames = np.arange(1, 41)
original = [0.795937, 1.721675, ...]  # 完整数据
decoded = [...]  # 解码结果

plt.figure(figsize=(12, 6))
plt.plot(frames, original, 'b-', label='Original', linewidth=2)
plt.plot(frames, decoded, 'r--', label='Decoded', linewidth=1.5)
plt.scatter([1, 34, 40], [0.795937, 71.693703, 90.674797], 
            c='green', s=100, label='Key Frames', zorder=5)
plt.xlabel('Frame')
plt.ylabel('translateZ Value')
plt.legend()
plt.grid(True, alpha=0.3)
plt.title('Animation Curve: Original vs Decoded')
plt.savefig('curve_comparison.png', dpi=300)
```

---
