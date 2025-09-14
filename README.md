# REE Path Searcher

A Rust tool for extracting file paths from RE Engine game PAK files and memory dumps.

**Acknowledgments**: This project is based on parts of the [mhrice](https://github.com/wwylele/mhrice) project. Special thanks to the original authors.

## Usage

```bash
# Extract paths from memory dump and PAK files
./ree-path-searcher.exe --dmp <memory_dump_file> --pak <pak_file_path>

# Multiple PAK files
./ree-path-searcher.exe --pak <pak_file_path_1> --pak <pak_file_path_2>
# or input a list file, each line is a PAK file path
./ree-path-searcher.exe --pak_list <pak_list_file>
```

## Library Usage

To use this as a library, refer to [src/main.rs](src/main.rs) for implementation examples.

---

# RE 引擎路径搜索器

用于从 RE Engine 游戏 PAK 文件和内存转储中提取文件路径的 Rust 工具。

**致谢**：本项目基于 [mhrice](https://github.com/wwylele/mhrice) 项目的部分代码修改而来，特此感谢原作者。

## 使用方法

```bash
# 从内存转储和PAK文件中提取路径
./ree-path-searcher.exe --dmp <memory_dump_file> --pak <pak_file_path>

# 多个PAK文件
./ree-path-searcher.exe --pak <pak_file_path_1> --pak <pak_file_path_2>
# 或输入一个列表文件，每行是一个PAK文件路径
./ree-path-searcher.exe --pak_list <pak_list_file>
```

## 作为库使用

如需作为库使用，请参考 [src/main.rs](src/main.rs) 文件中的实现示例。
