import json
from DataStorage import Packet, MutationResult, TestCase
from CalibrateData import MdsData
import binascii

from typing import List

def _to_printable(byte_data: List[int]) -> List[str]:
    """
    将字节数据转换为可打印的文本符号：
    - 可显示字符直接显示
    - 不可显示字符用 "." 表示
    """
    return [chr(b) if 32 <= b <= 126 else "." for b in byte_data]

class TestCaseProcessor:
    def __init__(self):
        return

    def parse_raw_data(self, raw_data: str) -> List[Packet]:
        byte_data = binascii.unhexlify(raw_data)
        index = 0
        packets = []

        while index < len(byte_data):
            if index + 2 > len(byte_data):
                print(f"数据不足以读取包长度，索引: {index}")
                break

            # 小端字节序读取包长度
            length = int.from_bytes(byte_data[index:index + 2], byteorder='little')
            index += 2

            if index + length > len(byte_data):
                print(f"数据不足以读取完整包，期望长度: {length}, 当前索引: {index}")
                break

            data = byte_data[index:index + length]
            packets.append(Packet(length=length, data=data))
            index += length

        return packets

    def parse_packets_cali_result(self, packets: List[Packet], packets_cali_result: List[dict]):
        for result in packets_cali_result:
            packet_id = result["packet_id"]
            offset = result["offset"]
            stable = result["stable"]
            mutation_operator = result["mutation_operator"]
            cf_index = result["cf_index"]
            cfc_index = result["cfc_index"]
            vf_index = result["vf_index"]
            

            mutation_result = MutationResult(offset=offset, cf_index=cf_index,cfc_index=cfc_index ,vf_index=vf_index, stable=stable)
            if packet_id < len(packets):
                packet = packets[packet_id]
                packet.mutations.setdefault(mutation_operator, []).append(mutation_result)

        for packet in packets:
            if "None" in packet.mutations:
                none_results = packet.mutations["None"]
                if none_results:
                    offset_0_result = none_results[0]
                    packet.mutations["None"] = [
                        MutationResult(offset=i,stable=stable, cf_index=offset_0_result.cf_index,cfc_index=cfc_index, vf_index=offset_0_result.vf_index)
                        for i in range(packet.length)
                    ]

    def generate_cf_vf_sequences(self, test_case: TestCase) -> MdsData:
        sequences = MdsData()

        for packet in test_case.packets:
            if "None" in packet.mutations:
                common_mutations = packet.mutations["None"]
            elif packet.mutations:
                common_mutations = list(packet.mutations.values())[0]
            else:
                common_mutations = []

            data_sequence = []
            for result in common_mutations:
                if result.offset < len(packet.data):
                    ch = _to_printable([packet.data[result.offset]])[0]
                else:
                    ch = "<out-of-range>"
                data_sequence.append(ch)

            sequences.add_sequence("Data", data_sequence)

            

            if "LBF" in packet.mutations:
                sequences.add_sequence("LBF_CF", [res.cf_index for res in packet.mutations["LBF"]])
                sequences.add_sequence("LBF_CFC", [res.cfc_index for res in packet.mutations["LBF"]])
                sequences.add_sequence("LBF_VF", [res.vf_index for res in packet.mutations["LBF"]])
                sequences.add_sequence("Stable", [res.stable for res in packet.mutations["LBF"]])
            if "FBF" in packet.mutations:
                sequences.add_sequence("FBF_CF", [res.cf_index for res in packet.mutations["FBF"]])
                sequences.add_sequence("FBF_CFC", [res.cfc_index for res in packet.mutations["FBF"]])
                sequences.add_sequence("FBF_VF", [res.vf_index for res in packet.mutations["FBF"]])
            if "ADD" in packet.mutations:
                sequences.add_sequence("ADD_CF", [res.cf_index for res in packet.mutations["ADD"]])
                sequences.add_sequence("ADD_CFC", [res.cfc_index for res in packet.mutations["ADD"]])
                sequences.add_sequence("ADD_VF", [res.vf_index for res in packet.mutations["ADD"]])
            if "SUB" in packet.mutations:
                sequences.add_sequence("SUB_CF", [res.cf_index for res in packet.mutations["SUB"]])
                sequences.add_sequence("SUB_CFC", [res.cfc_index for res in packet.mutations["SUB"]])
                sequences.add_sequence("SUB_VF", [res.vf_index for res in packet.mutations["SUB"]])
            if "None" in packet.mutations:
                sequences.add_sequence("Non_CF", [res.cf_index for res in packet.mutations["None"]])
                sequences.add_sequence("Non_CFC", [res.cfc_index for res in packet.mutations["None"]])
                sequences.add_sequence("Non_VF", [res.vf_index for res in packet.mutations["None"]])

        return sequences

    def parse_test_case(self, json_data: dict) -> TestCase:
        sequence_id = json_data["sequence_id"]
        raw_data = json_data.get("raw_data", "")
        packets = self.parse_raw_data(raw_data)

        packets_cali_result = json_data.get("packets_cali_result", [])
        self.parse_packets_cali_result(packets, packets_cali_result)

        return TestCase(sequence_id=sequence_id, packets=packets)
