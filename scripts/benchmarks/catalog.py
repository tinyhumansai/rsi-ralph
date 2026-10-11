"""Check published Euler solution availability without executing downloaded code."""
import concurrent.futures
import datetime
import hashlib
import html
import json
import re
import urllib.request

REVISION = '12f3dbc66ab9b7905c6cfe6f2fb4810583307923'
BASE = f'https://raw.githubusercontent.com/nayuki/Project-Euler-solutions/{REVISION}'
PROBLEMS = (186, 222, 280, 345, 387, 407, 425, 433, 500)


def parse_problem(number, page, source, answers, language):
    """Bind official metadata to an available solution and published answer."""
    title = re.search(rf'<title>#{number} (.*?) - Project Euler</title>', page)
    difficulty = re.search(r'Difficulty: Level (\d+) \[(\d+)%\]', page)
    answer = re.search(rf'^Problem {number}: ([^\r\n]+)$', answers, re.MULTILINE)
    if (not title or not difficulty or not answer
            or f'Solution to Project Euler problem {number}'.encode() not in source):
        raise ValueError('source metadata incomplete or mismatched')
    extension = 'java' if language == 'java' else 'py'
    return dict(problem=number, title=html.unescape(title[1]),
                problem_url=f'https://projecteuler.net/problem={number}',
                difficulty_percent=int(difficulty[2]), difficulty_level=int(difficulty[1]),
                solution_url=f'{BASE}/{language}/p{number:03d}.{extension}',
                solution_sha256=hashlib.sha256(source).hexdigest(),
                solution_language=language, published_answer=answer[1],
                answer_source_url=f'{BASE}/Answers.txt',
                answer_source_sha256=hashlib.sha256(answers.encode()).hexdigest(),
                source_available=True, reference_executed=False,
                classification='public control; possible training contamination')


def fetch(url):
    """Fetch bounded public provenance, failing on unavailable sources."""
    with urllib.request.urlopen(url, timeout=25) as response:
        body = response.read(1_000_001)
    if len(body) > 1_000_000:
        raise ValueError('source exceeds provenance size limit')
    return body


def main():
    """Print verified source catalog; solution availability is not execution evidence."""
    answers = fetch(f'{BASE}/Answers.txt').decode()

    def collect(number):
        language = 'java' if number == 433 else 'python'
        extension = 'java' if language == 'java' else 'py'
        source = fetch(f'{BASE}/{language}/p{number:03d}.{extension}')
        page = fetch(f'https://projecteuler.net/problem={number}').decode()
        return parse_problem(number, page, source, answers, language)

    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
        rows = list(executor.map(collect, PROBLEMS))
    print(json.dumps(dict(version=1, checked_at=datetime.datetime.now(datetime.UTC).isoformat(),
                          solution_revision=REVISION, problems=rows), indent=2))


if __name__ == '__main__':
    main()
