typedef unsigned long DWORD;
typedef long HRESULT;
typedef void *PTR;
__declspec(dllimport) PTR __stdcall LoadLibraryA(const char*);
__declspec(dllimport) PTR __stdcall GetProcAddress(PTR,const char*);
__declspec(dllimport) PTR __stdcall GetStdHandle(DWORD);
__declspec(dllimport) int __stdcall WriteFile(PTR,const void*,DWORD,DWORD*,PTR);
__declspec(dllimport) void __stdcall ExitProcess(unsigned int);
static void report(char *s) { DWORD n=0,w; while(s[n])n++; WriteFile(GetStdHandle((DWORD)-11),s,n,&w,0); }
void mainCRTStartup(void) {
    PTR d11=LoadLibraryA("d3d11.dll"),d12=LoadLibraryA("d3d12.dll");
    if(!d11 || !d12) { report("FAIL: Direct3D libraries did not load\r\n"); ExitProcess(1); }
    typedef HRESULT (__stdcall *Create11)(PTR,int,PTR,unsigned int,PTR,unsigned int,unsigned int,PTR*,unsigned int*,PTR*);
    Create11 create11=(Create11)GetProcAddress(d11,"D3D11CreateDevice");
    PTR device=0,context=0; unsigned int feature=0;
    if(!create11 || create11(0,1,0,0,0,0,7,&device,&feature,&context)<0) { report("FAIL: D3D11 device\r\n"); ExitProcess(2); }
    report("PASS: D3D11 hardware device created\r\n");
    struct GUID {unsigned int a; unsigned short b,c; unsigned char d[8];};
    struct GUID iid={0x189819f1,0x1db6,0x4b57,{0xbe,0x54,0x18,0x21,0x33,0x9b,0x85,0xf7}};
    typedef HRESULT (__stdcall *Create12)(PTR,unsigned int,struct GUID*,PTR*);
    Create12 create12=(Create12)GetProcAddress(d12,"D3D12CreateDevice");
    PTR device12=0;
    if(!create12 || create12(0,0xb000,&iid,&device12)<0) { report("FAIL: D3D12 device\r\n"); ExitProcess(3); }
    report("PASS: D3D12 hardware device created\r\n");
    ExitProcess(0);
}
