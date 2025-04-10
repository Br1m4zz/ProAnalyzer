from typing import Dict, List
import numpy as np

def isalnum(ch):
    # 判断字符是否为 a-z、A-Z 或 0-9
    return (('a' <= ch <= 'z') or ('A' <= ch <= 'Z') or ('0' <= ch <= '9'))

class MdsData:
    """
    封装测试结果序列（CF、VF和Data）的类
    """
    def __init__(self):
        self.data = []
        self.stable_mask = []

        self.cf_non = []
        self.cf_lbf = []
        self.cf_fbf = []
        self.cf_add = []
        self.cf_sub = []

        # self.cf_lbf_diff = []
        # self.cf_fbf_diff = []
        # self.cf_add_diff = []
        # self.cf_sub_diff = []

        self.cfc_non = []
        self.cfc_lbf = []
        self.cfc_fbf = []
        self.cfc_add = []
        self.cfc_sub = []

        # self.cfc_lbf_diff = []
        # self.cfc_fbf_diff = []
        # self.cfc_add_diff = []
        # self.cfc_sub_diff = []

        self.vf_non = []
        self.vf_lbf = []
        self.vf_fbf = []
        self.vf_add = []
        self.vf_sub = []

        # self.vf_lbf_diff = []
        # self.vf_fbf_diff = []
        # self.vf_add_diff = []
        # self.vf_sub_diff = []

    def add_sequence(self, key: str, sequence: List[int]):
        """
        添加序列到对应的字段
        """
        if key == "Data":
            self.data.append(sequence)
        elif key == "Non_CF":
            self.cf_non.append(sequence)
        elif key == "LBF_CF":
            self.cf_lbf.append(sequence)
        elif key == "FBF_CF":
            self.cf_fbf.append(sequence)
        elif key == "ADD_CF":
            self.cf_add.append(sequence)
        elif key == "SUB_CF":
            self.cf_sub.append(sequence)

        elif key == "Non_CFC":
            self.cfc_non.append(sequence)
        elif key == "LBF_CFC":
            self.cfc_lbf.append(sequence)
        elif key == "FBF_CFC":
            self.cfc_fbf.append(sequence)
        elif key == "ADD_CFC":
            self.cfc_add.append(sequence)
        elif key == "SUB_CFC":
            self.cfc_sub.append(sequence)

        elif key == "Non_VF":
            self.vf_non.append(sequence)
        elif key == "LBF_VF":
            self.vf_lbf.append(sequence)
        elif key == "FBF_VF":
            self.vf_fbf.append(sequence)
        elif key == "ADD_VF":
            self.vf_add.append(sequence)
        elif key == "SUB_VF":
            self.vf_sub.append(sequence)

        elif key == "Stable":
            self.stable_mask.append(sequence)

    def to_dict(self) -> Dict[str, List[List[int]]]:
        """
        将序列转换为字典格式
        """
        return {
            "Data": self.data,
            "Stable": self.stable_mask,

            "Non_CF": self.cf_non,
            "LBF_CF": self.cf_lbf,
            "FBF_CF": self.cf_fbf,
            "ADD_CF": self.cf_add,
            "SUB_CF": self.cf_sub,

            "Non_CFC": self.cfc_non,
            "LBF_CFC": self.cfc_lbf,
            "FBF_CFC": self.cfc_fbf,
            "ADD_CFC": self.cfc_add,
            "SUB_CFC": self.cfc_sub,

            "Non_VF": self.vf_non,
            "LBF_VF": self.vf_lbf,
            "FBF_VF": self.vf_fbf,
            "ADD_VF": self.vf_add,
            "SUB_VF": self.vf_sub
        }

    def print(self):
        """
        打印序列及其长度
        """
        print("\n生成的 CF 和 VF 序列及其长度:")
        for key, value in self.to_dict().items():
            print(f"{key}: {value} (长度: {len(value)})")

    def pos_stability(self, stable_mask):

        return [
            int(stable_mask[i]!=True)
            for i in range(len(stable_mask))
        ]

    def pos_sensitivity(self, non, lbf, fbf, add, sub):
        """
        计算每个位置的敏感度（四种算子差异计数）
        参数：
            non_cf: 基准序列 [int]
            lbf_cf: LBF算子序列 [int]
            fbf_cf: FBF算子序列 [int]
            add_cf: ADD算子序列 [int]
            sub_cf: SUB算子序列 [int]
        返回：
            sensitivity: 每个位置的差异计数列表 [int]
        """
        assert len({len(non), len(lbf), len(fbf), len(add), len(sub)}) == 1, "所有序列长度必须一致"
        
        return [
            sum([  # 统计该位置四种算子的差异总数
                int(lbf[i] != non[i]),
                int(fbf[i] != non[i]),
                int(add[i] != non[i]),
                int(sub[i] != non[i])
            ])
            for i in range(len(non))
        ]
    
    def pos_smilarity(self, lbf, fbf, add, sub):
        """
        计算每个位置四种算子之间的差异程度（成对比较差异对数）。
        
        参数：
        lbf: LBF算子序列 [int]
        fbf: FBF算子序列 [int]
        add: ADD算子序列 [int]
        sub: SUB算子序列 [int]
        
        """
        n = len(lbf)
        if not (len(fbf) == len(add) == len(sub) == n):
            raise ValueError("所有序列长度必须一致")
        
        simi = []
        for i in range(n):
            # 对于每个位置，获取四个算子的值
            values = [lbf[i], fbf[i], add[i], sub[i]]
            # 计算6个不同对中不同的对数量
            diff_count = 0
            for j in range(4):
                for k in range(j + 1, 4):
                    if values[j] != values[k]:
                        diff_count += 1
            simi.append(diff_count)

        return simi
       
    def dual_pos_consistency(self,m1,m2):
        """
        计算一致性：对每个位置，统计四个数值中出现次数最多的数量，
        最后返回总和/(4*位置个数)得到一致性比例。
        """
        assert len({len(m1), len(m2)}) == 1, "所有序列长度必须一致"
        return [
            sum([  # 统计该位置四种算子的差异总数
                int(m1[i] == m2[i]),
            ])
            for i in range(len(m1))
        ]

    def colorize_text(self, text, data):
        """ 根据 data 数组的数值 0, 1, 2, 3, 4 为 Raw_DT 进行颜色标记 """
        
        # 定义 ANSI 颜色代码
        COLORS = {
            0: "\033[0m",     # 复位颜色
            1: "\033[44m",   # 蓝色背景
            2: "\033[42m",   # 绿色背景
            3: "\033[43m",   # 黄色背景
            4: "\033[41m",   # 红色背景
            5: "\033[45m",   # 白色背景
            6: "\033[46m",   # 白色背景
            "reset": "\033[0m"  # 复位颜色
        }
        
        # 颜色化的文本
        colored_output = ""

        for i, char in enumerate(text):
            # 取当前索引位置对应的染色数值
            if i < len(data):  # 确保索引不越界
                value = data[i]
            else:
                value = 0  # 默认使用白色背景（0）
            
            color = COLORS.get(value, COLORS[0])  # 获取对应颜色，若无匹配则默认白色
            colored_output += f"{color}{char}{COLORS['reset']}"  # 拼接带颜色的字符

        return colored_output

    def colorize_segments(self, text, segments):
        """
        根据分段结果为 text 添加颜色，确保不同段用不同颜色展示
        """
        COLORS = [
            "\033[41m",  # 红色
            "\033[42m",  # 绿色
        ]

        colored_output = ""
        color_index = 0

        for segment in segments:
            color = COLORS[color_index % len(COLORS)]  # 循环使用颜色
            for i in segment:
                colored_output += f"{color}{text[i]}"

            colored_output += "\033[0m"  # 复位颜色
            color_index += 1  # 下一段使用新的颜色
        return colored_output

    def segment_input(self, cf_sen, cfc_sen, vf_sen, threshold=0.3):
        """
        根据 cf_sen, cfc_sen, vf_sen 构成的三维向量进行分段
        如果相邻向量的欧几里得距离超过 threshold，则划分为新段
        """
        segments = []
        current_segment = [0]  # 初始段从索引 0 开始

        for i in range(1, len(cf_sen)):
            # 计算相邻三维向量的欧几里得距离
            v1 = np.array([cf_sen[i-1], cfc_sen[i-1], vf_sen[i-1]])
            v2 = np.array([cf_sen[i], cfc_sen[i], vf_sen[i]])
            
            distance = np.linalg.norm(v1 - v2)  # 欧几里得距离计算
            if distance > threshold:
                segments.append(current_segment)  # 保存当前段
                current_segment = [i]  # 创建新段
            else:
                current_segment.append(i)

        segments.append(current_segment)  # 最后一个段加入列表
        
        return segments

    def generate_continue_mask(self, diff_list):
        """
        反映了字段在该算子变异的连续敏感度

        根据 diff_list 构造一个与其长度一致的 mask 序列：
        - diff_list 值为 0 的位置，mask 值为 0；
        - diff_list 值不为 0 的位置，交替赋值 1 和 2。
        
        参数：
            diff_list: 包含差值的数据列表，例如 [0, 0, 1, 1, 1, 0, 0, -1, -1]
        
        返回：
            mask: 一个等长于 diff_list 的列表，仅包含 0, 1, 2
                例如：
                输入  [0, 0, 1, 1, 1, 0, 0, -1, -1]
                输出  [0, 0, 1, 1, 1, 0, 0, 2, 2]
        """
        if not diff_list:
            return []
        mask = []
        nonzero_toggle = 1  # 用于交替赋值 1 和 2
        current_value = diff_list[0]

        for i, value in enumerate(diff_list):
            if value == 0:
                mask.append(0)
            else:
                if i == 0 or value != current_value:
                    nonzero_toggle = 2 if nonzero_toggle == 1 else 1  # 交替使用 1 和 2
                mask.append(nonzero_toggle)
            current_value = value  # 更新当前值
        return mask

    def colorize_field_by_mask(self, field, mask):
        """
        根据 mask 序列对 field 进行染色展示：
        - mask 值为 0 的部分保持原样，不染色；
        - mask 值为 1 的部分染为红色背景；
        - mask 值为 2 的部分染为绿色背景。

        参数:
        field: 需要染色的完整字符串或字符列表
        mask: 与 field 长度一致的掩码列表，包含 0、1、2

        返回:
        一个着色后的字符串（带有 ANSI 颜色码）。
        """
        # 如果 field 是字符列表，则转换为字符串
        if not isinstance(field, str):
            field = "".join(field)

        # 确保 mask 长度和 field 一致
        assert len(field) == len(mask), "mask 长度必须和 field 一致"

        colored_output = ""
        prev_color = None

        for i in range(len(field)):
            char = field[i]
            color = None

            if mask[i] == 1:
                color = "\033[41m"  # 红色背景
            elif mask[i] == 2:
                color = "\033[42m"  # 绿色背景
            elif mask[i] == 3:
                color = "\033[43m"  # 绿色背景

            if color:
                # 仅当颜色变化时才添加颜色标记，避免过多重复
                if color != prev_color:
                    colored_output += color
                colored_output += char
                prev_color = color
            else:
                # 颜色为 0，恢复默认状态
                if prev_color:
                    colored_output += "\033[0m"
                    prev_color = None
                colored_output += char

        # 结束时确保复位 ANSI 颜色代码
        if prev_color:
            colored_output += "\033[0m"

        print(f"res\t: {colored_output}")
        return colored_output

    def analyze_segment_masks(self,*masks):
        """
        反映了一组数据对某个流的敏感度

        根据 8 个 mask 输入，计算新的分段 mask：
        - 如果 8 个输入中全部为 0，则该偏移为 0；
        - 否则记作1        
        返回:
            new_mask: 计算出的新分段 mask（长度与输入 mask 相同）
        """
        # 确保所有 mask 长度一致
        mask_length = len(masks[0])
        assert all(len(mask) == mask_length for mask in masks), "所有输入 mask 必须长度一致"

        new_mask = []
        
        for i in range(mask_length):
            values_at_i = [mask[i] for mask in masks]
            
            if 0 in values_at_i:
                new_mask.append(0)  # 如果有任何一个 0，则染色为 0
            else:
                new_mask.append(1)  # 其他情况染色为 1

        return new_mask

    def combine_flow_masks(self, new_mask_cf, new_mask_vf):
        """
        综合控制流 (CF) 与值流 (VF) 的 mask 序列，生成新的分段 mask。
        
        规则：
        1. 如果对应偏移处 CF 和 VF 均为0，则输出0；
        2. 如果某偏移处仅一侧敏感（即一侧为0，另一侧为1或2），则输出该敏感值；
        3. 如果某偏移处CF和VF均敏感（即均不为0），则归为新的字段，输出3。
        
        参数：
        new_mask_cf: 控制流的 mask 序列，每个元素取值为 0, 1 或 2
        new_mask_vf: 值流的 mask 序列，每个元素取值为 0, 1 或 2
        
        返回：
        final_mask: 新的综合分段 mask 序列，其取值为0, 1, 2 或 3，其中3表示既CF敏感又VF敏感的情况。
        """
        if len(new_mask_cf) != len(new_mask_vf):
            raise ValueError("new_mask_cf and new_mask_vf must have the same length.")
        
        final_mask = []
        for cf_val, vf_val in zip(new_mask_cf, new_mask_vf):
            # 情况1：两者均为0
            if cf_val == 0 and vf_val == 0:
                final_mask.append(0)
            # 情况2：只有一侧敏感
            elif cf_val == 0 and vf_val != 0:
                final_mask.append(vf_val)
            elif cf_val != 0 and vf_val == 0:
                final_mask.append(cf_val)
            # 情况3：两者均敏感
            else:
                final_mask.append(3)
        return final_mask

    def segment_fields(self, raw, cf_sen, vf_sen, stable, 
                        cf_lbf, cf_fbf, cf_add, cf_sub):
        """   
        参数：
        raw: 原始数据文本（字符串或字符列表，用于展示）
        cf_sen: 控制流敏感度序列（取值范围 0～4）
        vf_sen: 值流敏感度序列（取值范围 0～4）
        stable: 稳定性序列（0 表示稳定，1 表示不稳定）
        cf_lbf: 最低位翻转 mask，一串数字，相同数字表示相同的控制流
        cf_fbf: 第一类算子序列（含义由业务定义）
        cf_add: 第二类算子序列
        cf_sub: 第三类算子序列
        
        返回：
        final_segments: 最终划分后的字段列表，每个字段表示为 (start_index, end_index)
        """
        # --- Step 0: 检查输入 ---
        n = len(cf_sen)
        if not (len(vf_sen) == len(stable) == len(cf_lbf) == len(cf_fbf) == 
                len(cf_add) == len(cf_sub) == n):
            raise ValueError("输入序列长度不一致")
        
        # --- Step 1: 粗粒度初步划分 —— 基于 cf_sen ---
        coarse_segments = []
        seg_start = 0
        current_sensitive = (cf_sen[0] == 4)
        for i in range(1, n):
            if (cf_sen[i] == 4) != current_sensitive:
                coarse_segments.append((seg_start, i - 1))
                seg_start = i
                current_sensitive = (cf_sen[i] == 4)
        coarse_segments.append((seg_start, n - 1))
        self.print_colored_segments(raw, coarse_segments)
        
        # --- 辅助函数：对某个算子序列进行详细分段 ---
        def get_boundaries(operator_seq, start, end):
            """
            返回候选分界点列表，K 满足 K in [start+2, end-2]，且
            operator_seq[K-2]==operator_seq[K-1]，operator_seq[K+1]==operator_seq[K+2]，
            且 operator_seq[K] 不等于 operator_seq[K-1] 和 operator_seq[K+1]。
            """
            boundaries = []
            # 注意：仅对满足左右上下文条件的区间处理
            for k in range(start+2, end-1):  # k 从 start+2 到 end-2（含）
                if (operator_seq[k-2] == operator_seq[k-1] and 
                    operator_seq[k+1] == operator_seq[k+2] and 
                    operator_seq[k] != operator_seq[k-1] and 
                    operator_seq[k] != operator_seq[k+1]):
                    boundaries.append(k)
            return boundaries

        # --- Step 2: 详细划分 —— 基于四种算子 (cf_lbf, cf_fbf, cf_add, cf_sub) ------------
        detailed_segments = []
        for (start, end) in coarse_segments:
            seg_start = start
            # 如果粗粒度段长度不足5，不进行详细划分
            if end - start + 1 < 5:
                detailed_segments.append((start, end))
                continue
            # 分别获取各算子的候选分界点
            b_lbf = get_boundaries(cf_lbf, start, end)
            b_fbf = get_boundaries(cf_fbf, start, end)
            b_add = get_boundaries(cf_add, start, end)
            b_sub = get_boundaries(cf_sub, start, end)
            # 构造候选边界计数字典
            boundary_counts = {}
            for b in b_lbf + b_fbf + b_add + b_sub:
                if b < start or b > end:
                    continue
                boundary_counts[b] = boundary_counts.get(b, 0) + 1
            
            # 整合：选择那些出现次数大于2（即>=3）的候选边界
            selected_boundaries = sorted([b for b, count in boundary_counts.items() if count > 2])
            
            # 根据选定的分界点对粗粒度段进行切分：将选定分界点处(即K)单独划为一段
            if not selected_boundaries:
                detailed_segments.append((start, end))
            else:
                current_start = start
                for b in selected_boundaries:
                    if current_start < b:
                        detailed_segments.append((current_start, b - 1))
                        # 将候选边界 b 单独为一段
                        detailed_segments.append((b, b))
                        current_start = b + 1
                detailed_segments.append((current_start, end))
        self.print_colored_segments(raw, detailed_segments)
        
        # --- Step 3: 根据 stable 合并不稳定字段 ---
        stable_segments = []
        cur_start, cur_end = detailed_segments[0]
        for (next_start, next_end) in detailed_segments[1:]:
            if stable[next_start] == 1 or stable[cur_end] == 1:
                cur_end = next_end
            else:
                stable_segments.append((cur_start, cur_end))
                cur_start, cur_end = next_start, next_end
        stable_segments.append((cur_start, cur_end))
        # self.print_colored_segments(raw, stable_segments)
        
        # --- Step 4: 根据 vf_sen 进一步划分 —— 按 vf_sen==0 与 非0 分段 ---
        final_segments = []
        for (start, end) in stable_segments:
            seg_start = start
            current_is_zero = (vf_sen[start] == 0)
            for i in range(start + 1, end + 1):
                flag = (vf_sen[i] == 0)
                # 当状态从 0 切换为非0或反之，划分出前一子段
                if flag != current_is_zero:
                    final_segments.append((seg_start, i - 1))
                    seg_start = i
                    current_is_zero = flag
            final_segments.append((seg_start, end))
        self.print_colored_segments(raw, final_segments)
        
        return final_segments

    def classify_and_color_segments(self, raw, segments, cf_sen, cfc_sen):
        """
        Parameters:
        raw (str): The raw text.
        segments (list of tuples): Each segment is represented as (start_index, end_index).
        cf_sen (list of int): Control-flow sensitivity values (one per position).
        cfc_sen (list of int): Additional CFC sensitivity values (one per position).
        
        Output:
        Prints the colored text.
        """
        # ANSI color codes:
        COLORS = {
            "CONTROL": "\033[41m",     # Red background
            "DELIMITER": "\033[43m",   # Yellow background
            "FLOW": "\033[44m",        # Blue background
            "DATA": "\033[40m",    # Black background
            "OTHER": "\033[42m"        # Green background
        }
        RESET = "\033[0m"
        
        colored_output = ""
        annotated_segments = []  # List of tuples: ((start, end), seg_type)
        
        for seg in segments:
            start, end = seg
            seg_len = end - start + 1
            
            # Compute the average cf_sen and cfc_sen over the segment.
            avg_cf = sum(cf_sen[i] for i in range(start, end + 1)) / seg_len
            avg_cfc = sum(cfc_sen[i] for i in range(start, end + 1)) / seg_len
            
            # Classification logic:
            if seg_len >= 2 and avg_cf == 4:
                seg_type = "CONTROL"
            elif seg_len <= 2 and  avg_cf == 4 and all(isalnum(ch)==False for ch in raw[start:end+1]) :
                seg_type = "DELIMITER"
            elif 0.5 < avg_cf < 4 and avg_cfc == 4:
                seg_type = "FLOW"
            # elif avg_cf <= 0.5:
            #     seg_type = "VARIABLE"
            else:
                seg_type = "DATA"
            
            annotated_segments.append(((start, end), seg_type))
            colored_segment = f"{COLORS.get(seg_type, COLORS['OTHER'])}{raw[start:end+1]}{RESET}"
            colored_output += colored_segment
        
        print("Colored text:")
        print(colored_output)
        # Optionally, uncomment below to print segment details:
        # for seg_range, seg_type in annotated_segments:
        #     s, e = seg_range
        #     print(f"Segment {seg_range}: '{raw[s:e+1]}' -> {seg_type}")
        return annotated_segments

    def print_colored_segments(self, data, segments):
        """
        根据划分的字段结果，在终端打印带颜色的数据展示。

        输入:
        data: 被划分的输入数据（字符串或列表）
        segments: 划分结果，每个字段表示为 (start_index, end_index)

        输出:
        无（仅在终端打印）
        """
        # 如果 data 是列表，则转换为字符串
        if isinstance(data, list):
            data = "".join(map(str, data))

        # 定义红色和绿色背景色代码
        COLORS = ["\033[41m", "\033[42m"]  # 红色和绿色交替
        RESET = "\033[0m"  # 颜色重置

        colored_output = ""

        for i, (start, end) in enumerate(segments):
            color = COLORS[i % len(COLORS)]  # 交替选择颜色
            colored_output += f"{color}{data[start:end + 1]}{RESET}"

        print(colored_output)

    def print_colored_segments_list(self, data, segments):
        """
        根据划分的字段结果，在终端打印带颜色的分段，每个分段显示为字符列表形式。

        输入:
        data: 被划分的输入数据（字符串或列表）。如果是列表，则先转换为字符串。
        segments: 分段结果，每个字段表示为 (start_index, end_index)

        输出:
        在终端打印各分段，格式类似:
        ['A', 'C', 'K'][' ']['s', 'i', 'p', ':']...
        """
        # 如果 data 是列表，则转换为字符串
        if isinstance(data, list):
            data = "".join(map(str, data))
            
        # 定义颜色（这里只使用两种交替颜色，比如红色和绿色背景）
        COLORS = ["\033[41m", "\033[42m"]
        RESET = "\033[0m"
        
        colored_output = ""
        for i, (start, end) in enumerate(segments):
            color = COLORS[i % len(COLORS)]
            segment_text = data[start:end+1]
            # 得到该分段对应的字符列表表示，例如 "ACK" -> ['A', 'C', 'K']
            seg_list_str = repr(list(segment_text))
            colored_output += f"{color}{seg_list_str}{RESET}"
            
        print(colored_output)


    def type_inference(self,divide_mask,cf_sen,cfc_sen,vf_sen):
        mask_length = len(divide_mask)
        new_mask = []

        """
        0 文本：                    divide_mask[i]指示为0，且cf_sen和vf_sen均指示为0
        1 ：                        divide_mask[i]指示不为，在cf_sen或者vf_sen指示不为0/ 在cf_sen
        2 用户、路径：              divide_maskp[i]指示为0，在在cfc_sen指示不为0
        3 控制字段、密码、分隔符：  divide_mask[i]指示为1，且cf_sen和vf_sen均指示为1
        3 
        """
        return new_mask

    def packet_analysis(self):
        output_rows = ["pkt,start,end,type"]
        print(len(self.data))
        for pkt in range(len(self.data)):
            if pkt >= len(self.cf_non):
                print(f"packet{pkt} time out happens!")
                print(f"{self.data}")
                return output_rows
            print(f"===========pkt:{pkt}==================")
            raw_dt = self.data[pkt]

            cf_sen = self.pos_sensitivity(
                self.cf_non[pkt],
                self.cf_lbf[pkt],
                self.cf_fbf[pkt],
                self.cf_add[pkt],
                self.cf_sub[pkt]
            )

            cfc_sen = self.pos_sensitivity(
                self.cfc_non[pkt],
                self.cfc_lbf[pkt],
                self.cfc_fbf[pkt],
                self.cfc_add[pkt],
                self.cfc_sub[pkt]
            )

            vf_sen = self.pos_sensitivity(
                self.vf_non[pkt],
                self.vf_lbf[pkt],
                self.vf_fbf[pkt],
                self.vf_add[pkt],
                self.vf_sub[pkt]
            )
                        
            # print(f"VF_Consistency: {vf_consistency}")
            st_mask = self.pos_stability(self.stable_mask[pkt])
            # print(self.cf_non[iter])
            # print(self.cf_lbf[iter])

            # # CF差分组，反映了与未变异测试的差异
            cf_lbf_diff = [lbf - non for lbf, non in zip(self.cf_lbf[pkt], self.cf_non[pkt])]
            cf_fbf_diff = [fbf - non for fbf, non in zip(self.cf_fbf[pkt], self.cf_non[pkt])]
            cf_add_diff = [add - non for add, non in zip(self.cf_add[pkt], self.cf_non[pkt])]
            cf_sub_diff = [sub - non for sub, non in zip(self.cf_sub[pkt], self.cf_non[pkt])]
            print(f"cf_non\t: {self.cf_non[pkt]}")
            print(f"cf_fbf_diff\t: {self.cf_fbf[pkt]}")
            # # CFC差分组，反映了与未变异测试的差异
            # cfc_lbf_diff = [lbf - non for lbf, non in zip(self.cfc_lbf[iter], self.cfc_non[iter])]
            # cfc_fbf_diff = [fbf - non for fbf, non in zip(self.cfc_fbf[iter], self.cfc_non[iter])]
            # cfc_add_diff = [add - non for add, non in zip(self.cfc_add[iter], self.cfc_non[iter])]
            # cfc_sub_diff = [sub - non for sub, non in zip(self.cfc_sub[iter], self.cfc_non[iter])]

            colored_raw_dt = self.colorize_text(raw_dt, cf_sen)
            print(f"cf_sen\t: {colored_raw_dt}")
            # colored_raw_dt = self.colorize_text(raw_dt, cfc_sen)
            # print(f"cfc_sen\t: {colored_raw_dt}")
            # colored_raw_dt = self.colorize_text(raw_dt, vf_sen)
            # print(f"vf_sen\t: {colored_raw_dt}")

            # print("##########")
            #反映了字段在该算子变异的连续程度+变异敏感程度（0,1,2）
            cf_lbf_diff_mask = self.generate_continue_mask(cf_lbf_diff)
            cf_fbf_diff_mask = self.generate_continue_mask(cf_fbf_diff)
            cf_add_diff_mask = self.generate_continue_mask(cf_add_diff)
            cf_sub_diff_mask = self.generate_continue_mask(cf_sub_diff)

            # cfc_lbf_diff_mask = self.generate_continue_mask(cfc_lbf_diff)
            # cfc_fbf_diff_mask = self.generate_continue_mask(cfc_fbf_diff) 
            # cfc_add_diff_mask = self.generate_continue_mask(cfc_add_diff)
            # cfc_sub_diff_mask = self.generate_continue_mask(cfc_sub_diff)

            #根据四种算子在CF和CFC的连续程度+敏感程度，得到控制流mask
            new_mask_cf = self.analyze_segment_masks(
                cf_lbf_diff_mask, cf_fbf_diff_mask, cf_add_diff_mask, cf_sub_diff_mask,
                # cfc_lbf_diff_mask, cfc_fbf_diff_mask, cfc_add_diff_mask, cfc_sub_diff_mask
            )

            # self.colorize_field_by_mask(raw_dt,new_mask_cf)

            print("=============================")

            ##############################
            #字段划分
            ##############################
            # print(cf_lbf_diff)
            # print(cf_fbf_diff)

            result = self.segment_fields(raw_dt,cf_sen,vf_sen,st_mask,self.cf_lbf[pkt],self.cf_fbf[pkt],self.cf_add[pkt],self.cf_sub[pkt])
            #step1: 首先使用新coverage敏感度指标的连续性、进行粗略划分

            #step2：使用VF敏感度指标的连续性进行二次划分

            #step3: 根据CF哈希向量的连续性进行三次划分

            ##############################
            #字段推理
            ##############################

            #step4：字段的综合CF敏感度、VF敏感度、循环敏感度、分段的长度,取值范围

            classified_segments = self.classify_and_color_segments(raw_dt,result,cf_sen,cfc_sen)
            for ((start, end), field_type) in classified_segments:
                # field_data = raw_dt[start:end+1].encode('utf-8').hex()  # Convert data to hexadecimal
                output_rows.append(f"{pkt},0x{start:04x},0x{end:04x},{field_type}")
        # for row in output_rows:
        #     print(row)
        return output_rows

