import json, tempfile, unittest, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
from check_thresholds import main

class CoverageTests(unittest.TestCase):
    def test_threshold(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "c.json"
            p.write_text(json.dumps({"data":[{"files":[{"filename":"crates/x/src/lib.rs","summary":{"lines":{"covered":8,"count":10}}}]}]}))
            self.assertEqual(main(str(p)), 0)

if __name__ == "__main__": unittest.main()
