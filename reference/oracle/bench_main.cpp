// Benchmark of the ORIGINAL MININEC3 engine, for comparison with the Rust
// port (crates/mmana-core/examples/bench.rs). Reuses the oracle's loader and
// times InitAntData() + CalCurrent() (segmentation, currents, patterns), the
// same work as mmana_core::calculate().
//
// usage: bench <file.maa>    prints "<pulses> <median ms>"
#define main oracle_main
#include "oracle_main.cpp"
#undef main

#include <algorithm>
#include <chrono>
#include <vector>

// TMainWnd::SetStackAnt + RunCurrentCalculation
static void Calculate(void)
{
	if( (ant.StackH == 1)&&(ant.StackV == 1) ){
		pCalAnt = &ant;
	}
	else {
		pCalAnt = &calAnt;
		Stack();
	}
	if( !InitAntData(pCalAnt) ){
		fprintf(stderr, "InitAntData failed\n");
		exit(1);
	}
	res.IncResP();
	res.GetResP()->TYPE = env.type;
	res.GetResP()->HEIGHT = env.antheight;
	res.GetResP()->FBR = env.fbr;
	res.GetResP()->RO = env.RO;
	res.GetResP()->JXO = env.JXO;
	CalCurrent();
}

static double Elapsed(void)
{
	auto t0 = std::chrono::steady_clock::now();
	Calculate();
	return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - t0).count();
}

int main(int argc, char **argv)
{
	if( argc < 2 ){
		fprintf(stderr, "usage: bench <file.maa>\n");
		return 2;
	}
	FILE *fp = fopen(argv[1], "rt");
	if( fp == NULL ){
		fprintf(stderr, "cannot open %s\n", argv[1]);
		return 2;
	}
	size_t cap = 1 << 20, len = 0;
	char *in = (char *)malloc(cap);
	char line[1024];
	in[0] = 0;
	while( fgets(line, 1024, fp) != NULL ){
		for( char *s = line; *s; s++ ){
			if( (*s == '\r') || (*s == '\n') ){ *s = 0; break; }
		}
		size_t l = strlen(line);
		if( len + l + 3 >= cap ) break;
		memcpy(in + len, line, l);
		len += l;
		in[len++] = '\r';
		in[len++] = '\n';
		in[len] = 0;
	}
	fclose(fp);

	InitEnv();
	env.pmax = InitNEC(env.pmax);
	InitAntDef();
	LoadAntStrings(in);
	free(in);
	char bf[64];
	double d;
	if( ant.fq ) snprintf(bf, sizeof(bf), "%.3lf", ant.fq); else bf[0] = 0;
	Calc(d, bf);
	if( d > 0 ) ant.fq = d;
	if( d > 0 ) ant.cfq = d;
	if( ant.cfq == 0.0 ) ant.cfq = ant.fq;

	// median over enough runs to fill about a second (as examples/bench.rs)
	double once = Elapsed();
	int runs = std::min(200, std::max(5, (int)(1000.0 / std::max(once, 1e-3))));
	std::vector<double> t;
	for( int i = 0; i < runs; i++ ) t.push_back(Elapsed());
	std::sort(t.begin(), t.end());
	printf("%d %.3f\n", GetPlusMax(), t[t.size()/2]);
	DeleteNEC();
	return 0;
}
