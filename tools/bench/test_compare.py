import json, tempfile, unittest
from pathlib import Path
from compare import main

class CompareTests(unittest.TestCase):
    def test_budget(self):
        with tempfile.TemporaryDirectory() as d:
            a, b = Path(d) / "a.json", Path(d) / "b.json"
            a.write_text(json.dumps({"ns_per_op": 100})); b.write_text(json.dumps({"ns_per_op": 104}))
            self.assertEqual(main(str(a), str(b)), 0)

if __name__ == "__main__": unittest.main()
