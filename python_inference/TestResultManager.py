from typing import List
from dataclasses import dataclass
from CalibrateData import MdsData
import os

@dataclass
class TestCaseResult:
    """
    表示单个测试用例结果的类
    """
    file_name: str
    calibrate: MdsData  # 使用封装的 Sequences 类


class TestResultsManager:
    """
    管理测试结果的类
    """
    def __init__(self):
        # 限定 testcases 是一个 TestCaseResult 对象的列表
        self.testcases: List[TestCaseResult] = []

    def add_result(self, file_name: str, sequences: MdsData):
        """
        添加测试用例结果到 testcases 列表中
        """
        if not isinstance(sequences, MdsData):
            raise TypeError("sequences 必须是 Sequences 类的实例")
        self.testcases.append(TestCaseResult(file_name=file_name, calibrate=sequences))

    def print_all_results(self):
        """
        打印所有测试结果
        """
        for testcase in self.testcases:
            print(f"\n文件: {testcase.file_name}")
            testcase.calibrate.print()

    def perform_in_packet_analysis(self,output_directory):
        for testcase in self.testcases:
            print(f"\n正在分析文件: {testcase.file_name}")
            
            # Obtain the CSV result from packet analysis.
            # We assume that packet_analysis() returns a list of CSV row strings,
            # but if it times out, it may return None.
            csv_rows = testcase.calibrate.packet_analysis()
            
            # If csv_rows is None (e.g., due to timeout), skip this test case.
            if csv_rows is None:
                print(f"Warning: No segmentation results for {testcase.file_name} due to timeout.")
                continue
            
            # Derive output file name from the source file name.
            base_name = os.path.basename(testcase.file_name)
            base_no_ext = os.path.splitext(base_name)[0]
            output_file_name = f"result_{base_no_ext}.csv"
            
            # Use the directory specified by the -i option; if self.input_dir is not set, default to current directory.
            output_path = os.path.join(output_directory, output_file_name)
            
            # Write the CSV rows to the output file.
            with open(output_path, "w", encoding="utf-8") as f:
                for row in csv_rows:
                    f.write(row + "\n")
                        
            print(f"CSV results saved to: {output_path}")
