import React from "react";
import { invoke } from '@tauri-apps/api/core';
import Image from 'next/image';
import AnalyticsConsentSwitch from "./AnalyticsConsentSwitch";


export function About() {
    const handleContactClick = async () => {
        try {
            await invoke('open_external_url', { url: 'https://meetily.zackriya.com/#about' });
        } catch (error) {
            console.error('Failed to open link:', error);
        }
    };

    return (
        <div className="p-4 space-y-4 h-[80vh] overflow-y-auto">
            {/* Compact Header */}
            <div className="text-center">
                <div className="mb-3">
                    <Image 
                        src="/glean-logo-blue.png" 
                        alt="Glean Logo" 
                        width={64} 
                        height={64}
                        className="mx-auto"
                    />
                </div>
                <h1 className="text-xl font-bold text-gray-900">Glean Meeting Minutes</h1>
                <span className="text-sm text-gray-500"> v0.0.5 - Pre Release</span>
                <p className="text-medium text-gray-600 mt-1">
                    AI-powered meeting transcription and summarization by Glean.
                </p>
            </div>

            {/* Features Grid - Compact */}
            <div className="space-y-3">
                <h2 className="text-base font-semibold text-gray-800">What makes Glean Meeting Minutes different</h2>
                <div className="grid grid-cols-2 gap-2">
                    <div className="bg-blue-50 rounded p-3 hover:bg-blue-100 transition-colors border border-blue-100">
                        <h3 className="font-bold text-sm text-blue-900 mb-1">Privacy-first</h3>
                        <p className="text-xs text-blue-700 leading-relaxed">Your data & AI processing workflow can now stay within your premise. No cloud, no leaks.</p>
                    </div>
                    <div className="bg-blue-50 rounded p-3 hover:bg-blue-100 transition-colors border border-blue-100">
                        <h3 className="font-bold text-sm text-blue-900 mb-1">Intelligent Search</h3>
                        <p className="text-xs text-blue-700 leading-relaxed">Powered by Glean's enterprise search capabilities for instant meeting insights and knowledge discovery.</p>
                    </div>
                    <div className="bg-blue-50 rounded p-3 hover:bg-blue-100 transition-colors border border-blue-100">
                        <h3 className="font-bold text-sm text-blue-900 mb-1">Cost-Smart</h3>
                        <p className="text-xs text-blue-700 leading-relaxed">Avoid pay-per-minute bills by running models locally (or pay only for the calls you choose).</p>
                    </div>
                    <div className="bg-blue-50 rounded p-3 hover:bg-blue-100 transition-colors border border-blue-100">
                        <h3 className="font-bold text-sm text-blue-900 mb-1">Works everywhere</h3>
                        <p className="text-xs text-blue-700 leading-relaxed">Google Meet, Zoom, Teams-online or offline.</p>
                    </div>
                </div>
            </div>

            {/* Coming Soon - Compact */}
            <div className="bg-blue-50 rounded p-3">
                <p className="text-s text-blue-800">
                    <span className="font-bold">Powered by Glean:</span> Advanced AI capabilities for intelligent meeting analysis, action tracking, and seamless knowledge integration.
                </p>
            </div>

            {/* CTA Section - Compact */}
            <div className="text-center space-y-2">
                <h3 className="text-medium font-semibold text-gray-800">Unlock the power of enterprise search</h3>
                <p className="text-s text-gray-600">
                    Experience how Glean transforms your organization's knowledge into <span className="font-bold">instant insights</span> and intelligent meeting summaries.
                </p>
                <button 
                    onClick={handleContactClick}
                    className="inline-flex items-center px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded transition-colors duration-200 shadow-sm hover:shadow-md"
                >
                    Learn more about Glean
                </button>
            </div>

            {/* Footer - Compact */}
            <div className="pt-2 border-t border-gray-200 text-center">
                <p className="text-xs text-gray-400">
                    Powered by Glean
                </p>
            </div>
            <AnalyticsConsentSwitch />
        </div>

    )
}