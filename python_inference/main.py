import CalibrateData
import DataStorage
import argparse, json, os
from TestCaseProcessor import TestCaseProcessor
from TestResultManager import TestResultsManager


def process_folder(input_folder: str) -> TestResultsManager:
    """
    处理指定文件夹中的所有 JSON 文件，返回 TestResultsManager 对象
    """
    processor = TestCaseProcessor()
    manager = TestResultsManager()
    json_files = [f for f in os.listdir(input_folder) if f.startswith("calibration_results_sequence_") and f.endswith(".json")]

    for json_file in json_files:
        file_path = os.path.join(input_folder, json_file)
        with open(file_path, 'r') as f:
            json_data = json.load(f)

        test_case = processor.parse_test_case(json_data)
        sequences = processor.generate_cf_vf_sequences(test_case)
        manager.add_result(json_file, sequences)

    return manager

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="处理指定文件夹中的 JSON 文件")
    parser.add_argument("-i", "--input", type=str, required=True, help="包含 JSON 文件的输入文件夹路径")
    args = parser.parse_args()

    # 获取所有测试结果
    manager: TestResultsManager = process_folder(args.input)
    output_directory = args.input
    # 打印所有结果
    manager.perform_in_packet_analysis(output_directory)